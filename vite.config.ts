import { defineConfig, type Plugin } from "vite";
import { extname, resolve, posix } from "node:path";
import { createReadStream, statSync, watch } from "node:fs";
import type { ServerResponse } from "node:http";

const DEFAULT_DEV_SERVER_PORT = 5073;

function readDevServerPort(): number {
  const parsed = Number.parseInt(process.env.VITE_PORT ?? "", 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : DEFAULT_DEV_SERVER_PORT;
}

const REFERENCE_MIME: Record<string, string> = {
  ".json": "application/json",
  ".zip": "application/zip",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".txt": "text/plain",
};

function lanPreviewPlugin(): Plugin {
  const enabled = process.env.VITE_LAN_RELOAD === "1";
  const distDir = resolve(__dirname, "dist");
  const referenceDir = resolve(__dirname, "reference");
  return {
    name: "mclone:lan-preview",
    transformIndexHtml: {
      order: "post",
      handler(html) {
        if (!enabled) return html;
        const tag = `<script>(()=>{const es=new EventSource("/__lan-reload");es.addEventListener("message",(e)=>{if(e.data==="reload")location.reload();});})();</script>`;
        return html.replace("</body>", `${tag}</body>`);
      },
    },
    configurePreviewServer(server) {
      if (!enabled) return;

      server.middlewares.use("/reference", (req, res, next) => {
        if (!req.url) {
          next();
          return;
        }
        const decoded = decodeURIComponent(req.url.split("?")[0] ?? "/");
        const rel = posix.normalize(decoded).replace(/^\/+/, "");
        const full = resolve(referenceDir, rel);
        if (full !== referenceDir && !full.startsWith(referenceDir + "/")) {
          res.statusCode = 403;
          res.end();
          return;
        }
        try {
          const s = statSync(full);
          if (!s.isFile()) {
            next();
            return;
          }
          res.setHeader("Content-Length", s.size);
          res.setHeader(
            "Content-Type",
            REFERENCE_MIME[extname(full).toLowerCase()] ?? "application/octet-stream",
          );
          res.setHeader("Cache-Control", "no-cache");
          createReadStream(full).pipe(res);
        } catch {
          next();
        }
      });

      const clients = new Set<ServerResponse>();
      let debounce: NodeJS.Timeout | null = null;

      try {
        watch(distDir, { recursive: true }, () => {
          if (debounce) return;
          debounce = setTimeout(() => {
            debounce = null;
            for (const res of clients) {
              try {
                res.write("data: reload\n\n");
              } catch {}
            }
          }, 200);
        });
      } catch (err) {
        console.error("[lan-reload] watch failed:", err);
      }

      server.middlewares.use("/__lan-reload", (req, res) => {
        res.writeHead(200, {
          "Content-Type": "text/event-stream",
          "Cache-Control": "no-cache, no-transform",
          Connection: "keep-alive",
          "X-Accel-Buffering": "no",
        });
        res.write("retry: 5000\n\n");
        const heartbeat = setInterval(() => {
          try {
            res.write(": ping\n\n");
          } catch {}
        }, 30000);
        clients.add(res);
        req.on("close", () => {
          clients.delete(res);
          clearInterval(heartbeat);
        });
      });
    },
  };
}

const devServerPort = readDevServerPort();

export default defineConfig({
  plugins: [lanPreviewPlugin()],
  server: {
    host: "0.0.0.0",
    port: devServerPort,
    strictPort: true,
    allowedHosts: ["mclone.graehlarts.com"],
    watch: {
      ignored: ["**/reference/**"],
    },
  },
  preview: {
    host: "0.0.0.0",
    port: devServerPort,
    strictPort: true,
    allowedHosts: ["mclone.graehlarts.com"],
  },
  build: {
    sourcemap: true,
    rollupOptions: {
      input: {
        index: resolve(__dirname, "index.html"),
        smoke: resolve(__dirname, "smoke.html"),
      },
    },
  },
});
