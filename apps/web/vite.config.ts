import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

const server = process.env.VEYRA_SERVER ?? 'http://127.0.0.1:4400';

export default defineConfig({
  plugins: [vue()],
  server: {
    port: Number(process.env.PORT ?? 5173),
    proxy: {
      '/api': server,
      '/ws': { target: server.replace(/^http/, 'ws'), ws: true },
      '/runner': { target: server.replace(/^http/, 'ws'), ws: true },
    },
  },
});
