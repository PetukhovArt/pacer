use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::connectors::secrets::{self, Lookup};
use crate::connectors::Connectors;
use crate::rpc::{Client, Outbox};
use crate::settings;

const TOKEN_ID: &str = "remote-pairing-token";
const COOKIE: &str = "berth_remote";
const COOKIE_MAX_AGE: u32 = 30 * 24 * 60 * 60;
const WS_PATH: &str = "/__berth/ws";
const HEALTH_PATH: &str = "/__berth/health";
const POLL: Duration = Duration::from_secs(1);
const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
const FAIL_WINDOW: Duration = Duration::from_secs(60);
const FAIL_LIMIT: u32 = 5;
const DEFAULT_PORT: u16 = 4477;
const INLINE: [&str; 9] = [
    "berth_watch",
    "berth_attach",
    "berth_input",
    "berth_resize",
    "berth_detach",
    "berth_mark_seen",
    "settings_watch",
    "fs_watch",
    "fs_unwatch",
];
const REMOTE_MARKER: &str = "<head><meta name=\"berth-remote\" content=\"1\">";
const DENIED_PAGE: &str = "<!doctype html><meta charset=utf-8><meta name=viewport content=\"width=device-width\"><title>Berth</title><body style=\"font-family:system-ui;padding:2rem;background:#111;color:#ddd\"><h1>Berth</h1><p>Open the pairing link or scan the QR code from Berth settings → Remote access.</p><p>Откройте ссылку подключения или отсканируйте QR-код в настройках Berth → Удалённый доступ.</p>";

pub struct WebAsset {
    pub bytes: Vec<u8>,
    pub mime: String,
}

pub type AssetFn = Arc<dyn Fn(&str) -> Option<WebAsset> + Send + Sync>;

pub enum WebSource {
    Proxy(String),
    Assets(AssetFn),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Bind {
    #[default]
    Localhost,
    Lan,
    Tailscale,
    #[serde(skip)]
    Fixed(IpAddr),
}

pub struct Pinned {
    pub addr: SocketAddr,
    pub public_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Config {
    enabled: bool,
    bind: Bind,
    port: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Problem {
    TailscaleMissing,
    BindFailed { message: String },
    TokenStore { message: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct ClientInfo {
    id: u64,
    address: String,
    agent: String,
    connected_at: u64,
}

#[derive(Debug, Serialize)]
pub struct RemoteStatus {
    enabled: bool,
    running: bool,
    problem: Option<Problem>,
    urls: Vec<String>,
    pairing_url: Option<String>,
    qr_svg: Option<String>,
    clients: Vec<ClientInfo>,
}

struct Server {
    addr: SocketAddr,
    urls: Vec<String>,
    task: JoinHandle<()>,
}

#[derive(Default)]
struct Live {
    enabled: bool,
    server: Option<Server>,
    problem: Option<Problem>,
    clients: HashMap<u64, ClientInfo>,
}

enum Auth {
    Granted,
    Denied,
    Blocked,
}

struct Shared {
    connectors: Arc<Connectors>,
    web: WebSource,
    http: reqwest::Client,
    token: Mutex<Option<String>>,
    live: Mutex<Live>,
    failures: Mutex<HashMap<IpAddr, (u32, Instant)>>,
    epoch: watch::Sender<u64>,
    next_client: AtomicU64,
    applying: tokio::sync::Mutex<()>,
    pinned: Option<Pinned>,
}

#[derive(Clone)]
pub struct Remote(Arc<Shared>);

#[derive(Clone)]
struct Ctx {
    shared: Arc<Shared>,
    epoch: u64,
}

fn guard<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn read_config() -> Config {
    let value: Value = serde_json::from_str(&settings::settings_text()).unwrap_or(Value::Null);
    let enabled = value
        .get("remote.enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let bind = value
        .get("remote.bind")
        .cloned()
        .and_then(|bind| serde_json::from_value(bind).ok())
        .unwrap_or_default();
    let port = value
        .get("remote.port")
        .and_then(Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port >= 1024)
        .unwrap_or(DEFAULT_PORT);
    Config {
        enabled,
        bind,
        port,
    }
}

fn route_source(target: Ipv4Addr) -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((target, 53)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_unspecified() && !ip.is_loopback() => Some(ip),
        _ => None,
    }
}

fn is_tailscale(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    a == 100 && (64..128).contains(&b)
}

fn tailscale_ip() -> Option<Ipv4Addr> {
    route_source(Ipv4Addr::new(100, 100, 100, 100)).filter(|ip| is_tailscale(*ip))
}

fn lan_ip() -> Option<Ipv4Addr> {
    route_source(Ipv4Addr::new(8, 8, 8, 8)).filter(|ip| !is_tailscale(*ip))
}

fn plan(config: Config) -> Result<(SocketAddr, Vec<String>), Problem> {
    let port = config.port;
    let url = |ip: Ipv4Addr| format!("http://{ip}:{port}/");
    let every = || {
        let mut urls: Vec<String> = lan_ip()
            .into_iter()
            .chain(tailscale_ip())
            .map(url)
            .collect();
        urls.push(url(Ipv4Addr::LOCALHOST));
        urls
    };
    match config.bind {
        Bind::Localhost => Ok((
            SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
            vec![url(Ipv4Addr::LOCALHOST)],
        )),
        Bind::Tailscale => tailscale_ip()
            .map(|ip| (SocketAddr::from((ip, port)), vec![url(ip)]))
            .ok_or(Problem::TailscaleMissing),
        Bind::Lan => Ok((SocketAddr::from((Ipv4Addr::UNSPECIFIED, port)), every())),
        Bind::Fixed(ip) => {
            let addr = SocketAddr::new(ip, port);
            let urls = if ip.is_unspecified() {
                every()
            } else {
                vec![format!("http://{addr}/")]
            };
            Ok((addr, urls))
        }
    }
}

async fn health() -> Response {
    plain(StatusCode::OK)
}

fn fresh_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| error.to_string())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

fn qr_svg(text: &str) -> Option<String> {
    use qrcode::render::svg;
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    Some(
        code.render::<svg::Color>()
            .min_dimensions(220, 220)
            .dark_color(svg::Color("#000000"))
            .light_color(svg::Color("#ffffff"))
            .build(),
    )
}

fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .find_map(|pair| {
            pair.trim()
                .strip_prefix(COOKIE)
                .and_then(|rest| rest.strip_prefix('='))
                .map(str::to_owned)
        })
}

fn query_token(uri: &Uri) -> Option<String> {
    uri.query()?
        .split('&')
        .find_map(|pair| pair.strip_prefix("token="))
        .map(str::to_owned)
}

fn same_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let host = headers
        .get(header::HOST)
        .and_then(|host| host.to_str().ok());
    let origin_host = origin
        .to_str()
        .ok()
        .and_then(|origin| origin.split_once("://"))
        .map(|(_, rest)| rest.trim_end_matches('/'));
    origin_host.is_some() && origin_host == host
}

fn plain(status: StatusCode) -> Response {
    status.into_response()
}

fn denied() -> Response {
    let mut response = (StatusCode::UNAUTHORIZED, DENIED_PAGE).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response
}

fn harden(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        header::HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    headers.insert(
        header::X_FRAME_OPTIONS,
        header::HeaderValue::from_static("DENY"),
    );
    response
}

impl Shared {
    fn config(&self) -> Config {
        match &self.pinned {
            Some(pinned) => Config {
                enabled: true,
                bind: Bind::Fixed(pinned.addr.ip()),
                port: pinned.addr.port(),
            },
            None => read_config(),
        }
    }

    fn public_urls(&self) -> Option<Vec<String>> {
        let url = self.pinned.as_ref()?.public_url.as_deref()?;
        Some(vec![format!("{}/", url.trim_end_matches('/'))])
    }

    fn token(&self) -> Result<String, String> {
        let mut slot = guard(&self.token);
        if let Some(token) = slot.as_ref() {
            return Ok(token.clone());
        }
        let token = match secrets::lookup(TOKEN_ID) {
            Lookup::Found(token) => token,
            Lookup::Missing => {
                let token = fresh_token()?;
                secrets::save(TOKEN_ID, &token)?;
                token
            }
            Lookup::Failed(message) => return Err(message),
        };
        *slot = Some(token.clone());
        Ok(token)
    }

    fn check(&self, ip: IpAddr, presented: &str) -> Auth {
        let mut failures = guard(&self.failures);
        let now = Instant::now();
        failures.retain(|_, (_, since)| now.duration_since(*since) < FAIL_WINDOW);
        if failures
            .get(&ip)
            .is_some_and(|(count, _)| *count >= FAIL_LIMIT)
        {
            return Auth::Blocked;
        }
        let granted = self.token().is_ok_and(|token| same(&token, presented));
        if granted {
            failures.remove(&ip);
            Auth::Granted
        } else {
            failures.entry(ip).or_insert((0, now)).0 += 1;
            Auth::Denied
        }
    }

    fn register(&self, ip: IpAddr, agent: String) -> u64 {
        let id = self.next_client.fetch_add(1, Ordering::Relaxed) + 1;
        guard(&self.live).clients.insert(
            id,
            ClientInfo {
                id,
                address: ip.to_string(),
                agent,
                connected_at: now_ms(),
            },
        );
        id
    }

    fn unregister(&self, id: u64) {
        guard(&self.live).clients.remove(&id);
    }

    fn stop(&self, live: &mut Live) {
        if let Some(server) = live.server.take() {
            server.task.abort();
        }
        self.epoch.send_modify(|epoch| *epoch += 1);
        live.clients.clear();
    }
}

impl Ctx {
    fn current(&self) -> bool {
        *self.shared.epoch.borrow() == self.epoch
    }
}

impl Remote {
    pub fn new(connectors: Arc<Connectors>, web: WebSource) -> Self {
        Self::build(connectors, web, None)
    }

    pub fn pinned(connectors: Arc<Connectors>, web: WebSource, pinned: Pinned) -> Self {
        Self::build(connectors, web, Some(pinned))
    }

    fn build(connectors: Arc<Connectors>, web: WebSource, pinned: Option<Pinned>) -> Self {
        Self(Arc::new(Shared {
            pinned,
            connectors,
            web,
            http: reqwest::Client::new(),
            token: Mutex::new(None),
            live: Mutex::new(Live::default()),
            failures: Mutex::new(HashMap::new()),
            epoch: watch::Sender::new(0),
            next_client: AtomicU64::new(0),
            applying: tokio::sync::Mutex::new(()),
        }))
    }

    pub fn spawn_watch(&self) {
        let remote = self.clone();
        tokio::spawn(async move {
            loop {
                remote.refresh().await;
                tokio::time::sleep(POLL).await;
            }
        });
    }

    pub async fn refresh(&self) {
        self.apply(self.0.config()).await;
    }

    pub fn shutdown(&self) {
        let mut live = guard(&self.0.live);
        if live.server.is_some() {
            self.0.stop(&mut live);
        }
    }

    async fn apply(&self, config: Config) {
        let shared = &self.0;
        let _applying = shared.applying.lock().await;
        let target = if config.enabled {
            Some(plan(config).and_then(|(addr, urls)| {
                shared
                    .token()
                    .map(|_| (addr, shared.public_urls().unwrap_or(urls)))
                    .map_err(|message| Problem::TokenStore { message })
            }))
        } else {
            None
        };
        let (addr, urls) = {
            let mut live = guard(&shared.live);
            live.enabled = config.enabled;
            match target {
                None => {
                    if live.server.is_some() {
                        shared.stop(&mut live);
                    }
                    live.problem = None;
                    return;
                }
                Some(Err(problem)) => {
                    if live.server.is_some() {
                        shared.stop(&mut live);
                    }
                    live.problem = Some(problem);
                    return;
                }
                Some(Ok((addr, urls))) => {
                    if let Some(server) = live.server.as_mut() {
                        if server.addr == addr {
                            server.urls = urls;
                            return;
                        }
                    }
                    if live.server.is_some() {
                        shared.stop(&mut live);
                    }
                    (addr, urls)
                }
            }
        };
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                let mut live = guard(&shared.live);
                shared.epoch.send_modify(|epoch| *epoch += 1);
                let ctx = Ctx {
                    shared: shared.clone(),
                    epoch: *shared.epoch.borrow(),
                };
                let app = Router::new()
                    .route(WS_PATH, get(socket))
                    .route(HEALTH_PATH, get(health))
                    .fallback(page)
                    .with_state(ctx);
                let task = tokio::spawn(async move {
                    let _ = axum::serve(
                        listener,
                        app.into_make_service_with_connect_info::<SocketAddr>(),
                    )
                    .await;
                });
                live.server = Some(Server { addr, urls, task });
                live.problem = None;
            }
            Err(error) => {
                guard(&shared.live).problem = Some(Problem::BindFailed {
                    message: format!("{addr}: {error}"),
                });
            }
        }
    }

    pub fn status(&self) -> RemoteStatus {
        let live = guard(&self.0.live);
        let urls = live
            .server
            .as_ref()
            .map(|server| server.urls.clone())
            .unwrap_or_default();
        let pairing_url = urls.first().and_then(|url| {
            let token = guard(&self.0.token).clone()?;
            Some(format!("{url}?token={token}"))
        });
        let mut clients: Vec<ClientInfo> = live.clients.values().cloned().collect();
        clients.sort_by_key(|client| client.id);
        RemoteStatus {
            enabled: live.enabled,
            running: live.server.is_some(),
            problem: live.problem.clone(),
            qr_svg: pairing_url.as_deref().and_then(qr_svg),
            pairing_url,
            urls,
            clients,
        }
    }

    pub fn pairing(&self) -> Result<String, String> {
        let status = self.status();
        status.pairing_url.ok_or_else(|| match status.problem {
            Some(Problem::BindFailed { message } | Problem::TokenStore { message }) => message,
            Some(Problem::TailscaleMissing) => "tailscale address not found".to_owned(),
            None => "remote server is not running".to_owned(),
        })
    }

    pub async fn regenerate(&self) -> Result<RemoteStatus, String> {
        let token = fresh_token()?;
        secrets::save(TOKEN_ID, &token)?;
        *guard(&self.0.token) = Some(token);
        guard(&self.0.failures).clear();
        {
            let mut live = guard(&self.0.live);
            if live.server.is_some() {
                self.0.stop(&mut live);
            }
        }
        self.refresh().await;
        Ok(self.status())
    }
}

async fn page(
    State(ctx): State<Ctx>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    if !ctx.current() {
        return plain(StatusCode::SERVICE_UNAVAILABLE);
    }
    if method != Method::GET && method != Method::HEAD {
        return plain(StatusCode::METHOD_NOT_ALLOWED);
    }
    let ip = peer.ip();
    if let Some(presented) = query_token(&uri) {
        return harden(match ctx.shared.check(ip, &presented) {
            Auth::Granted => Response::builder()
                .status(StatusCode::SEE_OTHER)
                .header(header::LOCATION, uri.path())
                .header(
                    header::SET_COOKIE,
                    format!(
                        "{COOKIE}={presented}; HttpOnly; SameSite=Strict; Path=/; Max-Age={COOKIE_MAX_AGE}"
                    ),
                )
                .body(Body::empty())
                .unwrap_or_else(|_| plain(StatusCode::INTERNAL_SERVER_ERROR)),
            Auth::Denied => denied(),
            Auth::Blocked => plain(StatusCode::TOO_MANY_REQUESTS),
        });
    }
    let Some(presented) = cookie_token(&headers) else {
        return harden(denied());
    };
    harden(match ctx.shared.check(ip, &presented) {
        Auth::Granted => serve(&ctx.shared, &uri, &headers).await,
        Auth::Denied => denied(),
        Auth::Blocked => plain(StatusCode::TOO_MANY_REQUESTS),
    })
}

fn page_body(mime: &str, bytes: Vec<u8>) -> Vec<u8> {
    if !mime.starts_with("text/html") {
        return bytes;
    }
    String::from_utf8_lossy(&bytes)
        .replacen("<head>", REMOTE_MARKER, 1)
        .into_bytes()
}

async fn serve(shared: &Shared, uri: &Uri, headers: &HeaderMap) -> Response {
    match &shared.web {
        WebSource::Proxy(base) => {
            let target = format!(
                "{}{}",
                base.trim_end_matches('/'),
                uri.path_and_query().map_or("/", |path| path.as_str())
            );
            let mut request = shared.http.get(target);
            if let Some(accept) = headers.get(header::ACCEPT) {
                request = request.header(header::ACCEPT, accept.clone());
            }
            let Ok(upstream) = request.send().await else {
                return plain(StatusCode::BAD_GATEWAY);
            };
            let status =
                StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let mime = upstream.headers().get(header::CONTENT_TYPE).cloned();
            let Ok(bytes) = upstream.bytes().await else {
                return plain(StatusCode::BAD_GATEWAY);
            };
            let kind = mime
                .as_ref()
                .and_then(|mime| mime.to_str().ok())
                .unwrap_or_default();
            let mut response = (status, page_body(kind, bytes.to_vec())).into_response();
            if let Some(mime) = mime {
                response.headers_mut().insert(header::CONTENT_TYPE, mime);
            }
            response
        }
        WebSource::Assets(resolve) => {
            let Some(asset) = resolve(uri.path()).or_else(|| resolve("/index.html")) else {
                return plain(StatusCode::NOT_FOUND);
            };
            let mut response = page_body(&asset.mime, asset.bytes).into_response();
            if let Ok(mime) = header::HeaderValue::from_str(&asset.mime) {
                response.headers_mut().insert(header::CONTENT_TYPE, mime);
            }
            response
        }
    }
}

async fn socket(
    State(ctx): State<Ctx>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !ctx.current() {
        return plain(StatusCode::SERVICE_UNAVAILABLE);
    }
    if !same_origin(&headers) {
        return plain(StatusCode::FORBIDDEN);
    }
    let ip = peer.ip();
    let authed = match cookie_token(&headers) {
        Some(presented) => match ctx.shared.check(ip, &presented) {
            Auth::Granted => true,
            Auth::Denied => false,
            Auth::Blocked => return plain(StatusCode::TOO_MANY_REQUESTS),
        },
        None => false,
    };
    let agent = headers
        .get(header::USER_AGENT)
        .and_then(|agent| agent.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    upgrade.on_upgrade(move |socket| session(ctx, socket, ip, agent, authed))
}

#[derive(Deserialize)]
struct Request {
    id: u64,
    cmd: String,
    #[serde(default)]
    args: Value,
}

async fn handle(client: &Client, request: Request) -> String {
    match client.call(&request.cmd, request.args).await {
        Ok(value) => json!({ "id": request.id, "ok": value }),
        Err(error) => json!({ "id": request.id, "err": error }),
    }
    .to_string()
}

async fn authenticate(ctx: &Ctx, socket: &mut WebSocket, ip: IpAddr) -> bool {
    let first = tokio::time::timeout(AUTH_TIMEOUT, socket.recv()).await;
    let presented = match first {
        Ok(Some(Ok(Message::Text(text)))) => serde_json::from_str::<Value>(text.as_str())
            .ok()
            .and_then(|value| value.get("auth").and_then(Value::as_str).map(str::to_owned)),
        _ => None,
    };
    presented.is_some_and(|token| matches!(ctx.shared.check(ip, &token), Auth::Granted))
}

async fn session(ctx: Ctx, mut socket: WebSocket, ip: IpAddr, agent: String, authed: bool) {
    if !authed && !authenticate(&ctx, &mut socket, ip).await {
        let _ = socket
            .send(Message::Close(Some(CloseFrame {
                code: 4401,
                reason: "unauthorized".into(),
            })))
            .await;
        return;
    }
    let (mut sink, mut stream) = socket.split();
    let (out, mut outgoing) = Outbox::new();
    let client = Arc::new(Client::new(ctx.shared.connectors.clone(), out.clone()));
    let id = ctx.shared.register(ip, agent);
    out.push(json!({ "ready": true }).to_string());
    let overflow = out.clone();
    let mut writer = tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                () = overflow.overflowed() => {
                    let close = Message::Close(Some(CloseFrame {
                        code: 1013,
                        reason: "output backlog overflow, reconnect".into(),
                    }));
                    let _ = tokio::time::timeout(Duration::from_secs(2), sink.send(close)).await;
                    break;
                }
                text = outgoing.recv() => {
                    let Some(text) = text else { break };
                    if sink.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    let mut epoch = ctx.shared.epoch.subscribe();
    loop {
        tokio::select! {
            _ = &mut writer => break,
            changed = epoch.changed() => {
                if changed.is_err() || *epoch.borrow() != ctx.epoch {
                    break;
                }
            }
            message = stream.next() => match message {
                Some(Ok(Message::Text(text))) => {
                    let Ok(request) = serde_json::from_str::<Request>(text.as_str()) else {
                        continue;
                    };
                    if INLINE.contains(&request.cmd.as_str()) {
                        out.push(handle(&client, request).await);
                    } else {
                        let client = client.clone();
                        let out = out.clone();
                        tokio::spawn(async move {
                            out.push(handle(&client, request).await);
                        });
                    }
                }
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            }
        }
    }
    ctx.shared.unregister(id);
    writer.abort();
}
