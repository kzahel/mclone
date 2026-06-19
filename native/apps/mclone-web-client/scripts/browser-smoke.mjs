import { chromium } from "@playwright/test";
import { spawnSync } from "node:child_process";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join, normalize, resolve, sep } from "node:path";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(scriptDir, "..");
const nativeRoot = resolve(appRoot, "../..");
const wwwRoot = join(appRoot, "www");
const wasmPath = join(
  nativeRoot,
  "target",
  "wasm32-unknown-unknown",
  "debug",
  "mclone_web_client.wasm",
);
const screenshotPath = process.env.MCLONE_NATIVE_WEB_SMOKE_SCREENSHOT
  ?? "/tmp/mclone-native-web-smoke.png";

run().catch((error) => {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
});

async function run() {
  buildWasm();
  const server = await startServer();
  let browser;
  try {
    const port = server.address().port;
    const baseUrl = `http://127.0.0.1:${port}`;
    browser = await chromium.launch({
      channel: process.env.PLAYWRIGHT_CHROME_CHANNEL ?? "chrome",
      headless: process.env.HEADED === "1" ? false : true,
      args: [
        "--enable-unsafe-webgpu",
        ...(process.platform === "darwin" ? ["--use-angle=metal"] : []),
      ],
    });
    const page = await browser.newPage();
    const pageErrors = [];
    page.on("pageerror", (error) => pageErrors.push(String(error)));
    page.on("console", (message) => {
      if (message.type() === "error") pageErrors.push(message.text());
    });

    await page.goto(baseUrl, { waitUntil: "load" });
    await page.waitForFunction(
      () => typeof globalThis.__mcloneNativeReady !== "undefined",
      undefined,
      { timeout: 20_000 },
    );
    const result = await page.evaluate(() => globalThis.__mcloneNativeReady);
    await page.screenshot({ path: screenshotPath, fullPage: true });

    assertSmokeResult(result, pageErrors);
    console.log(JSON.stringify({
      url: baseUrl,
      screenshotPath,
      result,
    }, null, 2));
  } finally {
    await browser?.close();
    await new Promise((resolveClose) => server.close(resolveClose));
  }
}

function buildWasm() {
  const result = spawnSync(
    "cargo",
    ["build", "-p", "mclone-web-client", "--target", "wasm32-unknown-unknown"],
    {
      cwd: nativeRoot,
      stdio: "inherit",
      env: process.env,
    },
  );
  if (result.status !== 0) {
    throw new Error(`cargo wasm build failed with status ${result.status ?? "unknown"}`);
  }
}

async function startServer() {
  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url ?? "/", "http://127.0.0.1");
      const path = resolveRequestPath(url.pathname);
      const bytes = await readFile(path);
      response.writeHead(200, {
        "Content-Type": contentType(path),
        "Cache-Control": "no-store",
      });
      response.end(bytes);
    } catch (error) {
      response.writeHead(error?.code === "ENOENT" ? 404 : 500, {
        "Content-Type": "text/plain; charset=utf-8",
      });
      response.end(error instanceof Error ? error.message : String(error));
    }
  });

  const requestedPort = Number.parseInt(process.env.MCLONE_NATIVE_WEB_SMOKE_PORT ?? "0", 10);
  await new Promise((resolveListen) => {
    server.listen(Number.isFinite(requestedPort) ? requestedPort : 0, "127.0.0.1", resolveListen);
  });
  return server;
}

function resolveRequestPath(pathname) {
  if (pathname === "/" || pathname === "/index.html") {
    return join(wwwRoot, "index.html");
  }
  if (pathname === "/mclone-web-smoke.js") {
    return join(wwwRoot, "mclone-web-smoke.js");
  }
  if (pathname === "/mclone_web_client.wasm") {
    return wasmPath;
  }

  const resolved = resolve(wwwRoot, `.${normalize(pathname)}`);
  if (resolved !== wwwRoot && !resolved.startsWith(`${wwwRoot}${sep}`)) {
    throw new Error(`refusing to serve path outside smoke root: ${pathname}`);
  }
  return resolved;
}

function contentType(path) {
  if (path.endsWith(".html")) return "text/html; charset=utf-8";
  if (path.endsWith(".js")) return "text/javascript; charset=utf-8";
  if (path.endsWith(".wasm")) return "application/wasm";
  return "application/octet-stream";
}

function assertSmokeResult(result, pageErrors) {
  if (pageErrors.length > 0) {
    throw new Error(`browser page errors:\n${pageErrors.join("\n")}`);
  }
  if (!result?.ok) {
    throw new Error(`native web smoke failed:\n${JSON.stringify(result, null, 2)}`);
  }
  if (!result.wasm?.report?.ok) {
    throw new Error(`wasm runtime report failed:\n${JSON.stringify(result.wasm, null, 2)}`);
  }
  if (
    result.wasm.report.commandCount !== 2
    || result.wasm.report.updateCount !== 4
    || !result.wasm.report.protocolCodecRoundtrip
  ) {
    throw new Error(`unexpected runtime message counts:\n${JSON.stringify(result.wasm.report, null, 2)}`);
  }
  if (
    result.wasm.report.loadedChunkCount !== 1
    || !result.wasm.report.centerChunkLoaded
    || !result.wasm.report.movedChunkLoaded
    || !result.wasm.report.previousChunkUnloaded
  ) {
    throw new Error(`client replica did not load center chunk:\n${JSON.stringify(result.wasm.report, null, 2)}`);
  }
  if (!result.webGpu?.ok) {
    throw new Error(`WebGPU probe did not produce a stable result:\n${JSON.stringify(result.webGpu, null, 2)}`);
  }
}
