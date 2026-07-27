import path from "node:path";
import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const webRoot = path.dirname(fileURLToPath(import.meta.url));
const textureLabRoot = path.resolve(webRoot, "..", "..");

export default defineConfig({
  root: webRoot,
  base: "/textures/",
  publicDir: false,
  plugins: [react()],
  server: {
    host: "127.0.0.1",
    port: 5177,
  },
  build: {
    outDir: path.join(textureLabRoot, "dist", "web"),
    emptyOutDir: true,
  },
});
