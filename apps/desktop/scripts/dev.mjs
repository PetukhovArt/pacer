import { spawn } from 'node:child_process';
import { homedir, tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const windows = process.platform === 'win32';
const devConfig = join(dirname(fileURLToPath(import.meta.url)), '..', 'src-tauri', 'tauri.dev.conf.json');
const devData = process.env.BERTH_DATA_DIR || join(homedir(), '.berth-dev', 'desktop');
const cdpPort = process.env.BERTH_CDP_PORT ?? process.env.VEYRA_CDP_PORT ?? '9334';

const env = {
  ...process.env,
  BERTH_RUNTIME_DIR: process.env.BERTH_RUNTIME_DIR || join(tmpdir(), 'berth-dev-desktop'),
  BERTH_DATA_DIR: devData,
  BERTH_HOME: process.env.BERTH_HOME || join(devData, 'home'),
  BERTH_SECRETS: process.env.BERTH_SECRETS || 'file',
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: process.env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS ?? `--remote-debugging-port=${cdpPort}`,
};

const child = spawn('pnpm', ['tauri', 'dev', '--config', windows ? `"${devConfig}"` : devConfig, ...process.argv.slice(2)], {
  env,
  stdio: 'inherit',
  shell: windows,
});

child.on('exit', (code) => process.exit(code ?? 0));
