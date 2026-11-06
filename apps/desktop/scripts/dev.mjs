import { spawn } from 'node:child_process';

const env = {
  ...process.env,
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:
    process.env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS ??
    `--remote-debugging-port=${process.env.BERTH_CDP_PORT ?? process.env.VEYRA_CDP_PORT ?? '9333'}`,
};

const child = spawn('pnpm', ['tauri', 'dev', ...process.argv.slice(2)], {
  env,
  stdio: 'inherit',
  shell: process.platform === 'win32',
});

child.on('exit', (code) => process.exit(code ?? 0));
