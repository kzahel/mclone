import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";
import { createServer } from "vite";

interface SmokeArgs {
  input: string;
  outPath: string;
}

const assetLabRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const args = parseArgs(process.argv.slice(2));
const figurePath = toVitePath(args.input);
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
    viewport: { width: 960, height: 720 },
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

  await page.goto(`${url}?figure=${encodeURIComponent(figurePath)}&clip=walk`, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.assetLabReady === true, undefined, { timeout: 15_000 });
  const previewError = await page.locator(".error").textContent().catch(() => null);
  if (previewError) {
    throw new Error(`Preview failed:\n${previewError}`);
  }
  if (browserErrors.length > 0) {
    throw new Error(`Browser errors:\n${browserErrors.join("\n")}`);
  }

  await page.screenshot({ path: args.outPath });
  console.log(`Wrote ${args.outPath}`);
} finally {
  await browser.close();
  await server.close();
}

function parseArgs(argv: string[]): SmokeArgs {
  const input = argv[0];
  if (!input || input.startsWith("-")) {
    throw new Error("Usage: tsx src/smoke.ts <figure.ts> [--out <png>]");
  }

  let outPath = path.join("/tmp", "mclone-asset-lab", "preview.png");
  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--out") {
      outPath = argv[index + 1] ?? outPath;
      index += 1;
    } else {
      throw new Error(`Unknown argument '${arg}'`);
    }
  }

  return { input, outPath };
}

function toVitePath(input: string): string {
  const absolute = path.resolve(input);
  const relative = path.relative(assetLabRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new Error(`Figure '${input}' must live under ${assetLabRoot}`);
  }
  return `/${relative.replaceAll(path.sep, "/")}`;
}
