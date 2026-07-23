import path from "node:path";
import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const webRoot = path.dirname(fileURLToPath(import.meta.url));
const terrainLabRoot = path.resolve(webRoot, "..", "..");

export default defineConfig({
  base: "/terrain/",
  root: webRoot,
  plugins: [react()],
  server: {
    host: "127.0.0.1",
    port: 5180,
  },
  preview: {
    host: "127.0.0.1",
    port: 4180,
  },
  build: {
    outDir: path.join(terrainLabRoot, "dist", "web"),
    emptyOutDir: true,
    rollupOptions: { input: path.join(webRoot, "index.html") },
  },
});
