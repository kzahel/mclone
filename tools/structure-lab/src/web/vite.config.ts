import path from "node:path";
import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const webRoot = path.dirname(fileURLToPath(import.meta.url));
const structureLabRoot = path.resolve(webRoot, "..", "..");

export default defineConfig({
  base: "/structures/",
  root: webRoot,
  publicDir: path.join(structureLabRoot, "dist", "catalog-public"),
  plugins: [react()],
  server: { host: "127.0.0.1", port: 5179 },
  preview: { host: "127.0.0.1", port: 4179 },
  build: {
    outDir: path.join(structureLabRoot, "dist", "web"),
    emptyOutDir: true,
    rollupOptions: { input: path.join(webRoot, "index.html") },
  },
});
