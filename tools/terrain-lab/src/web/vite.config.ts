import { createReadStream } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin } from "vite";

const webRoot = path.dirname(fileURLToPath(import.meta.url));
const terrainLabRoot = path.resolve(webRoot, "..", "..");
const repositoryRoot = path.resolve(terrainLabRoot, "..", "..");
const firstPartyPackRoot = path.join(
  repositoryRoot,
  "generated-assets",
  "first-party-stage",
  "first-party-packs",
);

export default defineConfig({
  base: "/terrain/",
  root: webRoot,
  plugins: [react(), localFirstPartyPacks()],
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

function localFirstPartyPacks(): Plugin {
  const install = (middlewares: {
    use: (handler: (
      request: import("node:http").IncomingMessage,
      response: import("node:http").ServerResponse,
      next: () => void,
    ) => void) => void;
  }): void => {
    middlewares.use((request, response, next) => {
      const pathname = new URL(request.url ?? "/", "http://terrain.local").pathname;
      const match = /^\/first-party-packs\/(mclone-(?:authored|generated-fallback)\.pbp)$/u
        .exec(pathname);
      if (!match?.[1]) {
        next();
        return;
      }
      response.statusCode = 200;
      response.setHeader("Content-Type", "application/octet-stream");
      response.setHeader("Cache-Control", "no-cache");
      const stream = createReadStream(path.join(firstPartyPackRoot, match[1]));
      stream.on("error", (error) => {
        response.statusCode = 500;
        response.end(`Failed to read first-party pack: ${error.message}`);
      });
      stream.pipe(response);
    });
  };
  return {
    name: "mclone-terrain-lab-first-party-packs",
    configureServer(server) {
      install(server.middlewares);
    },
    configurePreviewServer(server) {
      install(server.middlewares);
    },
  };
}
