import { defineConfig } from "vite";
import { resolve } from "node:path";

const DEFAULT_DEV_SERVER_PORT = 5670;

function readDevServerPort(): number {
  const parsed = Number.parseInt(process.env.VITE_PORT ?? "", 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : DEFAULT_DEV_SERVER_PORT;
}

export default defineConfig({
  server: {
    host: "0.0.0.0",
    port: readDevServerPort(),
    strictPort: true,
    allowedHosts: ["mclone.graehlarts.com"],
  },
  build: {
    rollupOptions: {
      input: {
        index: resolve(__dirname, "index.html"),
        debug: resolve(__dirname, "debug.html"),
        smoke: resolve(__dirname, "smoke.html"),
      },
    },
  },
});
