import http, { type Server } from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createServer as createViteServer } from "vite";
import { createTextureLabApi } from "./api";

const textureLabRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const viteConfigPath = path.join(textureLabRoot, "src", "web", "vite.config.ts");
const host = "127.0.0.1";
const port = parsePort(process.argv);

const vite = await createViteServer({
  configFile: viteConfigPath,
  server: { middlewareMode: true, host },
  appType: "spa",
});
const api = createTextureLabApi({
  textureLabRoot,
  inputPath: path.join(textureLabRoot, "packs", "mclone-default", "texture.ts"),
});

const server = http.createServer(async (request, response) => {
  try {
    if (await api(request, response)) {
      return;
    }
    vite.middlewares(request, response, (error: unknown) => {
      if (error) {
        vite.ssrFixStacktrace(error as Error);
        response.statusCode = 500;
        response.end(error instanceof Error ? error.message : String(error));
      } else {
        response.statusCode = 404;
        response.end("Not found");
      }
    });
  } catch (error) {
    vite.ssrFixStacktrace(error as Error);
    response.statusCode = 500;
    response.end(error instanceof Error ? error.message : String(error));
  }
});

const actualPort = await listenOnAvailablePort(server, port, host);
console.log(`Texture Lab UI: http://${host}:${actualPort}/`);

function parsePort(argv: string[]): number {
  const portArg = argv.find((arg) => arg.startsWith("--port="));
  if (!portArg) {
    return 5177;
  }
  const parsed = Number(portArg.slice("--port=".length));
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`Invalid --port value '${portArg}'`);
  }
  return parsed;
}

async function listenOnAvailablePort(server: Server, startPort: number, listenHost: string): Promise<number> {
  for (let candidate = startPort; candidate < startPort + 20; candidate += 1) {
    try {
      await listen(server, candidate, listenHost);
      return candidate;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "EADDRINUSE") {
        throw error;
      }
    }
  }
  throw new Error(`No available port found from ${startPort} to ${startPort + 19}`);
}

function listen(server: Server, candidatePort: number, listenHost: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const onError = (error: Error): void => {
      server.off("listening", onListening);
      reject(error);
    };
    const onListening = (): void => {
      server.off("error", onError);
      resolve();
    };
    server.once("error", onError);
    server.once("listening", onListening);
    server.listen(candidatePort, listenHost);
  });
}
