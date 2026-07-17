import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import { chromium } from "@playwright/test";
import { assetLabRoot } from "./vite-figure-path";

const execFileAsync = promisify(execFile);
const VIEW_NAMES = ["front", "right", "three-quarter"] as const;
const repoRoot = path.resolve(assetLabRoot, "../..");
const args = parseArgs(process.argv.slice(2));
const input = path.isAbsolute(args.input)
  ? args.input
  : path.resolve(repoRoot, args.input);
const relativeAssetPath = path.relative(repoRoot, input).split(path.sep).join("/");
if (relativeAssetPath.startsWith("../") || !relativeAssetPath.endsWith(".json")) {
  throw new Error(
    "Engine comparison requires a promoted figure JSON beneath the repository root",
  );
}
await fs.mkdir(args.outDir, { recursive: true });

await run(
  pnpmCommand(),
  [
    "--dir",
    assetLabRoot,
    "exec",
    "tsx",
    "src/review.ts",
    input,
    "--out-dir",
    args.outDir,
    "--width",
    String(args.width),
    "--height",
    String(args.height),
  ],
  repoRoot,
);
await run(
  cargoCommand(),
  [
    "run",
    "--manifest-path",
    "native/Cargo.toml",
    "-p",
    "mclone-figure-review",
    "--",
    "--asset-root",
    repoRoot,
    "--figure",
    relativeAssetPath,
    "--out-dir",
    args.outDir,
    "--review-contract",
    path.join(args.outDir, "review-contract.json"),
  ],
  repoRoot,
);

const threeReceipt = JSON.parse(
  await fs.readFile(path.join(args.outDir, "three-receipt.json"), "utf8"),
) as ThreeReceipt;
const engineReceipt = JSON.parse(
  await fs.readFile(path.join(args.outDir, "engine-receipt.json"), "utf8"),
) as EngineReceipt;
validateReceipts(threeReceipt, engineReceipt, args.width, args.height);
const comparisonPath = path.join(args.outDir, "comparison.png");
await writeComparisonSheet(comparisonPath, args.outDir, threeReceipt, engineReceipt);
const comparisonReceipt = {
  schemaVersion: 1,
  figure: engineReceipt.figure,
  semanticSha256: threeReceipt.semanticSha256,
  semanticCrc32: engineReceipt.semanticCrc32,
  compilerId: engineReceipt.compilerId,
  engineGeometryVariant: engineReceipt.geometryVariant,
  engineImageAlignment: "horizontal-reflection",
  comparison: path.basename(comparisonPath),
  views: VIEW_NAMES,
  review: engineReceipt.review,
  geometry: {
    parts: engineReceipt.partCount,
    vertices: engineReceipt.vertexCount,
    indices: engineReceipt.indexCount,
    drawRanges: engineReceipt.drawRangeCount,
    atlas: [engineReceipt.atlasWidth, engineReceipt.atlasHeight],
    cuboidProxies: engineReceipt.sphereCuboidProxyCount
      + engineReceipt.capsuleCuboidProxyCount
      + engineReceipt.cylinderCuboidProxyCount,
  },
  preparationMs: engineReceipt.preparationMs,
  immutableUploadCount: engineReceipt.immutableUploadCount,
  viewUniformWriteCount: engineReceipt.viewUniformWriteCount,
  checks: {
    figureIdentity: true,
    sharedReviewContract: true,
    expectedPreparedGeometry: true,
    oneResidencyUploadSet: true,
    oneViewWritePerPanel: true,
  },
};
await fs.writeFile(
  path.join(args.outDir, "comparison-receipt.json"),
  `${JSON.stringify(comparisonReceipt, null, 2)}\n`,
);
console.log(`Figure comparison wrote ${comparisonPath}`);

interface CompareArgs {
  input: string;
  outDir: string;
  width: number;
  height: number;
}

interface ReviewContract {
  panelWidth: number;
  panelHeight: number;
  fovDegrees: number;
  distance: number;
  target: [number, number, number];
  background: string;
}

interface ThreeReceipt {
  schemaVersion: number;
  figure: string;
  semanticSha256: string;
  semanticJsonBytes: number;
  contract: ReviewContract;
  views: string[];
}

interface EngineReceipt {
  schemaVersion: number;
  figure: string;
  assetPath: string;
  compilerId: string;
  geometryVariant: "exact-box" | "cuboid-proxy";
  semanticCrc32?: string;
  preparationMs: number;
  partCount: number;
  vertexCount: number;
  indexCount: number;
  drawRangeCount: number;
  boxPrimitiveCount: number;
  sphereCuboidProxyCount: number;
  capsuleCuboidProxyCount: number;
  cylinderCuboidProxyCount: number;
  atlasWidth: number;
  atlasHeight: number;
  review: ReviewContract;
  immutableUploadCount: number;
  viewUniformWriteCount: number;
}

function parseArgs(argv: string[]): CompareArgs {
  let input = path.join(repoRoot, "assets/mclone/figures/player.figure.json");
  let outDir = "/tmp/mclone-figure-compare/player";
  let width = 360;
  let height = 480;
  let positionalSeen = false;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--") {
      continue;
    } else if (argument === "--out-dir") {
      outDir = requiredValue(argument, argv[index + 1]);
      index += 1;
    } else if (argument === "--width") {
      width = parseDimension(argument, argv[index + 1]);
      index += 1;
    } else if (argument === "--height") {
      height = parseDimension(argument, argv[index + 1]);
      index += 1;
    } else if (argument?.startsWith("-")) {
      throw new Error(`Unknown argument '${argument}'`);
    } else if (argument && !positionalSeen) {
      input = argument;
      positionalSeen = true;
    } else {
      throw new Error(`Unexpected argument '${argument}'`);
    }
  }
  return { input, outDir: path.resolve(outDir), width, height };
}

function requiredValue(flag: string, value: string | undefined): string {
  if (!value) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
}

function parseDimension(flag: string, value: string | undefined): number {
  const parsed = Number.parseInt(requiredValue(flag, value), 10);
  if (!Number.isInteger(parsed) || parsed <= 0 || parsed > 4096) {
    throw new Error(`${flag} must be an integer from 1 through 4096`);
  }
  return parsed;
}

async function run(command: string, commandArgs: string[], cwd: string): Promise<void> {
  const { stdout, stderr } = await execFileAsync(command, commandArgs, {
    cwd,
    maxBuffer: 16 * 1024 * 1024,
  });
  if (stdout.trim()) {
    process.stdout.write(stdout);
  }
  if (stderr.trim()) {
    process.stderr.write(stderr);
  }
}

function pnpmCommand(): string {
  return process.platform === "win32" ? "pnpm.cmd" : "pnpm";
}

function cargoCommand(): string {
  return process.platform === "win32" ? "cargo.exe" : "cargo";
}

function validateReceipts(
  three: ThreeReceipt,
  engine: EngineReceipt,
  width: number,
  height: number,
): void {
  if (three.schemaVersion !== 1 || engine.schemaVersion !== 1) {
    throw new Error("Figure review receipt schema mismatch");
  }
  if (three.figure !== engine.figure) {
    throw new Error(`Figure review identity mismatch: '${three.figure}' versus '${engine.figure}'`);
  }
  if (!contractsAgree(three.contract, engine.review)) {
    throw new Error("Three.js and engine review contracts differ");
  }
  if (three.contract.panelWidth !== width || three.contract.panelHeight !== height) {
    throw new Error("Figure review output dimensions differ from the requested dimensions");
  }
  if (
    engine.vertexCount !== engine.partCount * 24
    || engine.indexCount !== engine.partCount * 36
    || engine.drawRangeCount !== engine.partCount * 6
  ) {
    throw new Error(
      `Prepared ${engine.figure} reported inconsistent cuboid geometry: `
      + `${engine.partCount} parts, ${engine.vertexCount} vertices, `
      + `${engine.indexCount} indices, and ${engine.drawRangeCount} ranges`,
    );
  }
  const proxyCount = engine.sphereCuboidProxyCount
    + engine.capsuleCuboidProxyCount
    + engine.cylinderCuboidProxyCount;
  if (engine.boxPrimitiveCount + proxyCount !== engine.partCount) {
    throw new Error("Prepared primitive accounting does not match the part count");
  }
  const expectedVariant = proxyCount === 0 ? "exact-box" : "cuboid-proxy";
  if (engine.geometryVariant !== expectedVariant) {
    throw new Error(
      `Prepared geometry variant '${engine.geometryVariant}' should be '${expectedVariant}'`,
    );
  }
  if (engine.immutableUploadCount !== 4 || engine.viewUniformWriteCount !== VIEW_NAMES.length) {
    throw new Error("Prepared figure did not retain immutable resources across review views");
  }
  if (three.views.join(",") !== VIEW_NAMES.join(",")) {
    throw new Error("Asset Lab review view order changed");
  }
}

function contractsAgree(left: ReviewContract, right: ReviewContract): boolean {
  return left.panelWidth === right.panelWidth
    && left.panelHeight === right.panelHeight
    && close(left.fovDegrees, right.fovDegrees)
    && close(left.distance, right.distance)
    && left.target.every((value, index) => close(value, right.target[index] ?? Number.NaN))
    && left.background === right.background;
}

function close(left: number, right: number): boolean {
  return Number.isFinite(left) && Number.isFinite(right) && Math.abs(left - right) <= 1e-5;
}

async function writeComparisonSheet(
  outPath: string,
  outDir: string,
  three: ThreeReceipt,
  engine: EngineReceipt,
): Promise<void> {
  const images = new Map<string, string>();
  for (const renderer of ["three", "engine"] as const) {
    for (const view of VIEW_NAMES) {
      const bytes = await fs.readFile(path.join(outDir, `${renderer}-${view}.png`));
      images.set(`${renderer}-${view}`, `data:image/png;base64,${bytes.toString("base64")}`);
    }
  }
  const panelWidth = three.contract.panelWidth;
  const panelHeight = three.contract.panelHeight;
  const rows = VIEW_NAMES.map((view) => `
    <section class="view-row">
      <h2>${escapeHtml(viewLabel(view))}</h2>
      <div class="pair">
        <figure><figcaption>Three.js semantic</figcaption><img src="${images.get(`three-${view}`)}"></figure>
        <figure class="engine"><figcaption>${engineCaption(engine)}</figcaption><img src="${images.get(`engine-${view}`)}"></figure>
      </div>
    </section>
  `).join("");
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({
      viewport: {
        width: panelWidth * 2 + 80,
        height: (panelHeight + 66) * VIEW_NAMES.length + 130,
      },
      deviceScaleFactor: 1,
    });
    await page.setContent(`<!doctype html>
      <html><head><style>
        * { box-sizing: border-box; }
        html, body { margin: 0; background: #dfe7ed; color: #17202a; font-family: system-ui, sans-serif; }
        #comparison { width: ${panelWidth * 2 + 56}px; margin: 12px; padding: 16px; background: #f8fafc; }
        header { margin-bottom: 12px; }
        h1 { margin: 0 0 4px; font-size: 21px; }
        header p { margin: 0; color: #475569; font: 12px ui-monospace, monospace; }
        .view-row { margin: 0 0 14px; }
        h2 { margin: 0 0 5px; font-size: 14px; text-transform: uppercase; letter-spacing: 0.04em; }
        .pair { display: grid; grid-template-columns: ${panelWidth}px ${panelWidth}px; gap: 8px; }
        figure { margin: 0; border: 1px solid #9aaab8; background: #edf1f4; }
        figcaption { height: 38px; padding: 4px 7px; background: #dbe5ec; font-size: 12px; font-weight: 700; }
        img { display: block; width: ${panelWidth}px; height: ${panelHeight}px; image-rendering: auto; }
        .engine img { transform: scaleX(-1); }
        footer { color: #475569; font-size: 11px; line-height: 1.35; }
      </style></head><body>
        <main id="comparison">
          <header>
            <h1>${escapeHtml(engine.figure)} — semantic vs ${escapeHtml(engine.geometryVariant)}</h1>
            <p>${escapeHtml(engine.compilerId)} · ${engine.partCount} parts · ${engine.vertexCount} vertices · ${engine.indexCount} indices · ${engine.preparationMs.toFixed(3)} ms preparation</p>
          </header>
          ${rows}
          <footer>Engine panels are horizontally reflected only in this sheet to align the deliberate Three.js −Z-front → engine +Z-front handedness conversion. Raw engine PNGs remain unmodified beside this comparison.</footer>
        </main>
      </body></html>`);
    await page.locator("#comparison").screenshot({ path: outPath });
  } finally {
    await browser.close();
  }
}

function engineCaption(engine: EngineReceipt): string {
  if (engine.geometryVariant === "cuboid-proxy") {
    const proxies = engine.sphereCuboidProxyCount
      + engine.capsuleCuboidProxyCount
      + engine.cylinderCuboidProxyCount;
    return `Mclone prepared cuboid proxy (${proxies} approximated parts, view-aligned)`;
  }
  return "Mclone prepared exact boxes (view-aligned)";
}

function viewLabel(view: typeof VIEW_NAMES[number]): string {
  return view === "three-quarter" ? "Three-quarter" : view[0]?.toUpperCase() + view.slice(1);
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}
