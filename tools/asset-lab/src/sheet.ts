import fs from "node:fs/promises";
import path from "node:path";
import { chromium } from "@playwright/test";
import { createServer } from "vite";
import { assetLabRoot, toViteFigurePath } from "./vite-figure-path";

interface SheetArgs {
  input: string;
  outPath: string;
  clip: string;
  debug: boolean;
  staticPose: boolean;
}

const args = parseArgs(process.argv.slice(2));
const figurePath = toViteFigurePath(args.input);
await fs.mkdir(path.dirname(args.outPath), { recursive: true });

const server = await createServer({
  root: assetLabRoot,
  logLevel: "error",
  server: {
    host: "127.0.0.1",
    port: 0,
    hmr: false,
    strictPort: false,
  },
});

await server.listen();

const url = server.resolvedUrls?.local[0];
if (!url) {
  throw new Error("Vite did not report a local URL");
}

const browser = await chromium.launch();
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 960 },
    deviceScaleFactor: 1,
  });
  const browserErrors: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      browserErrors.push(message.text());
    }
  });
  page.on("pageerror", (error) => {
    browserErrors.push(error.stack ?? error.message);
  });

  const params = new URLSearchParams({
    figure: figurePath,
    clip: args.clip,
    debug: args.debug ? "1" : "0",
    static: args.staticPose ? "1" : "0",
  });
  await page.goto(`${url}sheet.html?${params.toString()}`, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.assetLabSheetReady === true, undefined, { timeout: 15_000 });
  const previewError = await page.locator(".error").textContent().catch(() => null);
  if (previewError) {
    throw new Error(`Sheet render failed:\n${previewError}`);
  }
  if (browserErrors.length > 0) {
    throw new Error(`Browser errors:\n${browserErrors.join("\n")}`);
  }

  await page.locator("#sheet").screenshot({ path: args.outPath });
  console.log(`Wrote ${args.outPath}`);
} finally {
  await browser.close();
  await server.close();
}

function parseArgs(argv: string[]): SheetArgs {
  const input = argv[0];
  if (!input || input.startsWith("-")) {
    throw new Error("Usage: tsx src/sheet.ts <figure.ts|figure.json> [--out <png>] [--clip <name>] [--clean]");
  }

  let outPath = path.join("/tmp", "mclone-asset-lab", "sheet.png");
  let clip = "walk";
  let debug = true;
  let staticPose = false;
  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--out") {
      outPath = argv[index + 1] ?? outPath;
      index += 1;
    } else if (arg === "--clip") {
      clip = argv[index + 1] ?? clip;
      index += 1;
    } else if (arg === "--clean") {
      debug = false;
    } else if (arg === "--static") {
      staticPose = true;
    } else {
      throw new Error(`Unknown argument '${arg}'`);
    }
  }

  return { input, outPath, clip, debug, staticPose };
}
