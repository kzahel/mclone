import path from "node:path";
import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const webRoot = path.dirname(fileURLToPath(import.meta.url));
const assetLabRoot = path.resolve(webRoot, "..", "..");

export default defineConfig({
  base: "/animals/",
  root: webRoot,
  publicDir: path.join(assetLabRoot, "dist", "catalog-public"),
  plugins: [react()],
  server: {
    host: "127.0.0.1",
    port: 5178,
  },
  preview: {
    host: "127.0.0.1",
    port: 4178,
  },
  build: {
    outDir: path.join(assetLabRoot, "dist", "web"),
    emptyOutDir: true,
  },
});
