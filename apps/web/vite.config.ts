import { existsSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

const dataDir = process.env.VEYRA_DATA_DIR ?? resolve(import.meta.dirname, '../../.data');
const devKeyFile = join(dataDir, 'dev-user.key');
const runnerFile = join(dataDir, 'runner.json');

export default defineConfig(({ command }) => ({
  plugins: [vue()],
  define: {
    __VEYRA_RELAY__: JSON.stringify(process.env.VEYRA_RELAY ?? 'ws://localhost:3000'),
    __VEYRA_DEV_KEY__: JSON.stringify(command === 'serve' && existsSync(devKeyFile) ? readFileSync(devKeyFile, 'utf8').trim() : null),
    __VEYRA_RUNNER__: command === 'serve' && existsSync(runnerFile) ? readFileSync(runnerFile, 'utf8') : 'null',
  },
  server: {
    port: Number(process.env.PORT ?? 5173),
  },
}));
