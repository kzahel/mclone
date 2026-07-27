import { spawnSync } from "node:child_process";
import { createReadStream } from "node:fs";
import { stat, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, devices } from "@playwright/test";

import { resolveBrowserWebGpuLaunch } from "../../../../scripts/browser-webgpu-env.mjs";

const scriptRoot = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(scriptRoot, "..");
const nativeRoot = path.resolve(appRoot, "../..");
const repositoryRoot = path.resolve(nativeRoot, "..");
const webRoot = path.join(nativeRoot, "target", "mclone-world-explorer-www");
const mobile = process.argv.includes("--mobile");
const skipBuild = process.argv.includes("--skip-build");
const semanticOnly = process.argv.includes("--semantic-only");
const composition = argumentValue("--composition") ?? "composed";
if (!["composed", "coverage", "exact"].includes(composition)) {
  throw new Error(`unsupported browser composition smoke mode ${composition}`);
}
const externalBaseUrl = process.env.WORLD_EXPLORER_SMOKE_BASE_URL;
const label = mobile ? "phone" : "desktop";
const port = Number.parseInt(
  process.env.WORLD_EXPLORER_SMOKE_PORT ?? (mobile ? "4194" : "4193"),
  10,
);
const launch = resolveBrowserWebGpuLaunch();
const pageErrors = [];
let server;
let browser;
let page;

try {
  if (!skipBuild && !externalBaseUrl) {
    run("node", [path.join(appRoot, "scripts", "build-web.mjs")]);
  }
  if (!externalBaseUrl) {
    server = await startServer();
  }
  browser = await chromium.launch({
    channel: process.env.PLAYWRIGHT_CHROME_CHANNEL ?? "chrome",
    headless: launch.headless,
    args: ["--enable-unsafe-webgpu", ...launch.chromeArgs],
    env: { ...process.env, ...launch.browserEnv },
  });
  const context = await browser.newContext(
    mobile
      ? { ...devices["Pixel 7"], isMobile: true }
      : { viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 },
  );
  page = await context.newPage();
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") {
      pageErrors.push(message.text());
    }
  });
  const target = new URL(
    "?seed=12345&centerX=0&centerZ=0&blocksAcross=96"
      + "&view=3d&projection=perspective&yaw=3.1415927&pitch=0.12"
      + `&composition=${composition}&exactRadius=2&smokeObserver=1`,
    normalizedBaseUrl(),
  );
  await page.goto(target.href, { waitUntil: "networkidle" });
  await page.locator("#world-explorer-shell").waitFor({ state: "visible" });
  await page.waitForFunction(
    () => globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer,
  );
  await page.waitForFunction(() => {
    const report =
      globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer.snapshot();
    return report?.targetReady === true
      && report?.exactComplete === true
      && report?.pendingRefills === 0
      && report?.readySlots === report?.allocationSlots;
  }, null, { timeout: 240_000 });
  await page.waitForTimeout(250);
  const report = await page.evaluate(
    () => globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__.observer.snapshot(),
  );
  assertCompositionReport(report);
  const capture = semanticOnly
    ? null
    : `/tmp/mclone-world-explorer-web-${label}-${composition}-review.png`;
  if (capture) {
    await page.locator("#world-explorer-canvas").screenshot({ path: capture });
  }
  if (pageErrors.length > 0) {
    throw new Error(`browser errors:\n${pageErrors.join("\n")}`);
  }
  const receipt = {
    artifacts: {
      javascriptBytes: (await stat(
        path.join(webRoot, "pkg", "mclone_world_explorer.js"),
      )).size,
      wasmBytes: (await stat(
        path.join(webRoot, "pkg", "mclone_world_explorer_bg.wasm"),
      )).size,
    },
    capture,
    launch: {
      headed: launch.headed,
      useWayland: launch.useWayland,
      waylandDisplay: launch.waylandDisplay,
    },
    report,
    target: label,
    url: target.href,
  };
  const receiptPath =
    `/tmp/mclone-world-explorer-web-${label}-${composition}-receipt.json`;
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ receipt: receiptPath, ...receipt }, null, 2));
} catch (error) {
  const failureCapture =
    `/tmp/mclone-world-explorer-web-${label}-${composition}-failure.png`;
  await page?.screenshot({ path: failureCapture, fullPage: true }).catch(() => {});
  const runtime = await page?.evaluate(() => ({
    report:
      globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__?.observer.snapshot() ?? null,
    status: document.getElementById("world-explorer-status")?.textContent ?? "",
  })).catch(() => null);
  console.error(JSON.stringify({
    browserErrors: pageErrors,
    failureCapture,
    runtime,
  }, null, 2));
  throw error;
} finally {
  await browser?.close();
  await new Promise((resolve) => server?.close(resolve) ?? resolve());
}

function assertCompositionReport(report) {
  const expectedCoverageMode = composition === "coverage"
    ? "visualize-painted"
    : composition === "composed"
      ? "discard-painted"
      : "disabled";
  if (report.composition !== composition
      || report.exactRadius !== 2
      || report.exactDesiredChunks !== 25
      || report.exactPaintedChunks !== 25
      || report.exactQueuedChunks !== 0
      || report.exactPendingAdmissions !== 0
      || report.exactInFlight
      || !report.exactComplete
      || report.exactCoverageMode !== expectedCoverageMode
      || (composition !== "exact"
        && report.proceduralCoverageGeneration !== report.exactCoverageGeneration)
      || (composition !== "exact" && report.proceduralPaintedChunks !== 25)
      || report.treeProxyMissingExactRecords !== 0
      || report.treeProxyMissingProxyRecords !== 0
      || report.canonicalExactOwnedTreeRecords
        + report.canonicalProxyOwnedTreeRecords !== report.exactNaturalTreeRecords
      || report.exactResidentMeshBytes <= 0
      || report.exactVertexCount <= 0
      || report.exactIndexCount <= 0
      || report.readySlots !== report.allocationSlots
      || report.pendingRefills !== 0
      || !report.targetReady) {
    throw new Error(
      `browser exact composition did not reach a coherent frame:\n`
        + `${JSON.stringify(report, null, 2)}`,
    );
  }
}

function argumentValue(name) {
  const inline = process.argv.find((argument) => argument.startsWith(`${name}=`));
  if (inline) {
    return inline.slice(name.length + 1);
  }
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

function normalizedBaseUrl() {
  const baseUrl = externalBaseUrl ?? `http://127.0.0.1:${port}/`;
  return baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`;
}

async function startServer() {
  const localServer = createServer((request, response) => {
    const requested = new URL(request.url ?? "/", `http://127.0.0.1:${port}`);
    const relative = requested.pathname === "/"
      ? "index.html"
      : decodeURIComponent(requested.pathname.replace(/^\/+/u, ""));
    const file = path.resolve(webRoot, relative);
    if (!file.startsWith(`${webRoot}${path.sep}`)
        && file !== path.join(webRoot, "index.html")) {
      response.writeHead(403).end();
      return;
    }
    response.setHeader("Cross-Origin-Opener-Policy", "same-origin");
    response.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
    response.setHeader("Cross-Origin-Resource-Policy", "same-origin");
    response.setHeader("Cache-Control", "no-store");
    response.setHeader("Content-Type", contentType(file));
    const stream = createReadStream(file);
    stream.on("error", () => response.writeHead(404).end("not found"));
    stream.pipe(response);
  });
  await new Promise((resolve, reject) => {
    localServer.once("error", reject);
    localServer.listen(port, "127.0.0.1", resolve);
  });
  return localServer;
}

function contentType(file) {
  switch (path.extname(file)) {
    case ".css": return "text/css; charset=utf-8";
    case ".html": return "text/html; charset=utf-8";
    case ".js": return "text/javascript; charset=utf-8";
    case ".wasm": return "application/wasm";
    default: return "application/octet-stream";
  }
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: repositoryRoot,
    env: process.env,
    stdio: "inherit",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${command} failed with status ${result.status ?? "unknown"}`);
  }
}
