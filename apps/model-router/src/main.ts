import { createServer, type IncomingMessage, type ServerResponse } from 'node:http'
import { readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'
import { Readable } from 'node:stream'
import { ProxyAgent, fetch } from 'undici'
import { parse } from 'yaml'

type Provider = { type: 'anthropic'; base: string; key?: string; auth?: 'x-api-key' | 'bearer' | 'passthrough'; compat?: boolean }
type Route = { match?: string; use: string }
type Config = { providers: Record<string, Provider>; models: Record<string, string[]>; routes: Route[] }

const configPath = process.env.BERTH_ROUTES ?? process.env.VEYRA_ROUTES ?? join(homedir(), '.berth', 'routes.yaml')
const port = Number(process.env.BERTH_ROUTER_PORT ?? process.env.VEYRA_ROUTER_PORT ?? 4466)
const clientKey = process.env.BERTH_ROUTER_KEY ?? process.env.VEYRA_ROUTER_KEY
const proxy = process.env.HTTPS_PROXY ?? process.env.https_proxy
if (!proxy) throw new Error('HTTPS_PROXY is required: the router never calls upstream without it')
const dispatcher = new ProxyAgent(proxy)
const retryable = new Set([408, 429, 500, 502, 503, 504, 529])

function loadConfig(): Config {
  return parse(readFileSync(configPath, 'utf8')) as Config
}

function chainFor(config: Config, requested: string): { route: string; targets: string[] } {
  for (const route of config.routes) {
    if (route.match && !new RegExp(route.match, 'i').test(requested)) continue
    const targets = config.models[route.use] ?? [route.use]
    return { route: route.match ?? 'default', targets }
  }
  throw new Error(`no route for ${requested}`)
}

const hopByHop = new Set(['host', 'connection', 'content-length', 'accept-encoding', 'transfer-encoding', 'keep-alive'])

function upstreamHeaders(req: IncomingMessage, provider: Provider): Record<string, string> {
  if (provider.auth === 'passthrough') {
    const headers: Record<string, string> = {}
    for (const [name, value] of Object.entries(req.headers)) {
      if (!hopByHop.has(name) && typeof value === 'string') headers[name] = value
    }
    return headers
  }
  const key = provider.key && process.env[provider.key]
  if (!key) throw new Error(`env ${provider.key} is empty`)
  const headers: Record<string, string> = { 'content-type': 'application/json' }
  for (const name of provider.compat ? ['anthropic-version'] : ['anthropic-version', 'anthropic-beta']) {
    const value = req.headers[name]
    if (typeof value === 'string') headers[name] = value
  }
  headers['anthropic-version'] ??= '2023-06-01'
  if (provider.auth === 'bearer') headers.authorization = `Bearer ${key}`
  else headers['x-api-key'] = key
  return headers
}

type Message = { role: string; content: string | { type: string }[] }

function blocks(content: Message['content']) {
  return typeof content === 'string' ? [{ type: 'text', text: content }] : content
}

function plainAnthropic(body: Record<string, unknown>) {
  const rest = { ...body }
  for (const field of ['thinking', 'output_config', 'context_management']) delete rest[field]
  const messages: Message[] = []
  for (const message of body.messages as Message[]) {
    const role = message.role === 'system' ? 'user' : message.role
    const previous = messages.at(-1)
    if (previous?.role === role) previous.content = [...blocks(previous.content), ...blocks(message.content)]
    else messages.push({ role, content: message.content })
  }
  return { ...rest, messages }
}

async function readBody(req: IncomingMessage): Promise<string> {
  const chunks: Buffer[] = []
  for await (const chunk of req) chunks.push(chunk as Buffer)
  return Buffer.concat(chunks).toString('utf8')
}

function send(res: ServerResponse, status: number, body: unknown) {
  res.writeHead(status, { 'content-type': 'application/json' })
  res.end(JSON.stringify(body))
}

function authorized(req: IncomingMessage): boolean {
  if (!clientKey) return true
  const bearer = req.headers.authorization?.replace(/^Bearer\s+/i, '')
  return bearer === clientKey || req.headers['x-api-key'] === clientKey
}

async function handle(req: IncomingMessage, res: ServerResponse) {
  const path = (req.url ?? '').split('?')[0] ?? ''
  if (req.method !== 'POST' || !path.startsWith('/v1/messages')) return send(res, 404, { error: 'not found' })
  if (!authorized(req)) return send(res, 401, { error: 'unauthorized' })

  const raw = await readBody(req)
  const body = JSON.parse(raw)
  const config = loadConfig()
  const { route, targets } = chainFor(config, String(body.model ?? ''))
  let last = { status: 502, text: 'no upstream answered' }

  for (const target of targets) {
    const slash = target.indexOf('/')
    const provider = config.providers[target.slice(0, slash)]
    const model = target.slice(slash + 1) === '*' ? body.model : target.slice(slash + 1)
    if (!provider) throw new Error(`unknown provider in ${target}`)
    const started = Date.now()
    try {
      const upstream = await fetch(provider.base.replace(/\/$/, '') + path, {
        method: 'POST',
        headers: upstreamHeaders(req, provider),
        body: JSON.stringify({ ...(provider.compat ? plainAnthropic(body) : body), model }),
        dispatcher,
      })
      console.error(`${path} requested=${body.model} route=${route} -> ${target} ${upstream.status} ${Date.now() - started}ms`)
      if (retryable.has(upstream.status)) {
        last = { status: upstream.status, text: await upstream.text() }
        continue
      }
      if (path.endsWith('/count_tokens') && !upstream.ok) break
      res.writeHead(upstream.status, { 'content-type': upstream.headers.get('content-type') ?? 'application/json' })
      if (upstream.body) Readable.fromWeb(upstream.body as never).pipe(res)
      else res.end()
      return
    } catch (error) {
      console.error(`${path} requested=${body.model} route=${route} -> ${target} network error ${(error as Error).message}`)
      last = { status: 502, text: (error as Error).message }
    }
  }

  if (path.endsWith('/count_tokens')) return send(res, 200, { input_tokens: Math.ceil(raw.length / 4) })
  send(res, last.status, { type: 'error', error: { type: 'api_error', message: last.text } })
}

createServer((req, res) => {
  handle(req, res).catch((error: Error) => {
    console.error(error.message)
    if (!res.headersSent) send(res, 500, { type: 'error', error: { type: 'api_error', message: error.message } })
    else res.end()
  })
}).listen(port, '127.0.0.1', () => console.error(`model-router on http://127.0.0.1:${port}, config ${configPath}, proxy ${proxy}`))
