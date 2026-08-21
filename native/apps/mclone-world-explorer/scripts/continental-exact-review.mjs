import { spawnSync } from "node:child_process";
import { mkdir, readFile, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptRoot = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(scriptRoot, "..");
const nativeRoot = path.resolve(appRoot, "../..");
const repositoryRoot = path.resolve(nativeRoot, "..");
const binary = path.join(nativeRoot, "target", "debug", "mclone-world-explorer");
const outputRoot = "/tmp/mclone-continental-exact-review";
const skipBuild = process.argv.includes("--skip-build");
const skipBrowser = process.argv.includes("--skip-browser");
const exactRadius = 4;
const sites = [
  {
    label: "clearing",
    journey: "clearing-between-forest-cores",
  },
  {
    label: "water",
    journey: "connected-water-country",
    centerX: "7168",
    centerZ: "-21504",
  },
  {
    label: "arid",
    journey: "upland-to-arid-basin",
  },
];
const frames = [
  { label: "exact", composition: "exact", blocksAcross: 96, pitch: 0.52 },
  { label: "coverage", composition: "coverage", blocksAcross: 512, pitch: 0.42 },
  { label: "composed", composition: "composed", blocksAcross: 512, pitch: 0.42 },
  { label: "horizon", composition: "composed", blocksAcross: 4096, pitch: 0.34 },
];

await mkdir(outputRoot, { recursive: true });
if (!skipBuild) {
  run("cargo", [
    "build",
    "--manifest-path", path.join(nativeRoot, "Cargo.toml"),
    "-p", "mclone-worldgen",
    "--bin", "mclone_continental_exact_review",
    "-p", "mclone-world-explorer",
  ]);
}

const directReceipt = path.join(outputRoot, "direct-exact.json");
run(path.join(nativeRoot, "target", "debug", "mclone_continental_exact_review"), [
  "--output", directReceipt,
]);

const nativeCaptures = [];
const nativeSmokes = [];
for (const site of sites) {
  const siteArgs = reviewSiteArgs(site);
  for (const frame of frames) {
    const capture = path.join(outputRoot, `${site.label}-${frame.label}.png`);
    run(binary, [
      ...siteArgs,
      "--view", "3d",
      "--pitch", frame.pitch.toString(),
      "--blocks-across", frame.blocksAcross.toString(),
      "--composition", frame.composition,
      "--exact-radius", exactRadius.toString(),
      "--capture", capture,
    ]);
    nativeCaptures.push(await artifact(capture));
  }
  const smokeRoot = path.join(outputRoot, `${site.label}-smoke`);
  run(binary, [
    ...siteArgs,
    "--view", "3d",
    "--pitch", "0.20",
    "--blocks-across", "512",
    "--composition", "composed",
    "--exact-radius", exactRadius.toString(),
    "--retained-smoke", smokeRoot,
  ]);
  nativeSmokes.push({
    label: site.label,
    offscreen: JSON.parse(await readFile(path.join(smokeRoot, "offscreen", "receipt.json"))),
    window: JSON.parse(await readFile(path.join(smokeRoot, "window", "receipt.json"))),
  });
}

const browserReceipts = [];
if (!skipBrowser) {
  if (!skipBuild) {
    run("node", [path.join(scriptRoot, "build-web.mjs")]);
  }
  for (const site of sites) {
    const argumentsForBrowser = [
      path.join(scriptRoot, "browser-composition-smoke.mjs"),
      "--skip-build",
      "--source", "continental",
      "--journey", site.journey,
      "--composition", "composed",
      "--blocks-across", "512",
      "--exact-radius", exactRadius.toString(),
    ];
    if (site.centerX !== undefined) {
      argumentsForBrowser.push("--center-x", site.centerX, "--center-z", site.centerZ);
    }
    run("node", argumentsForBrowser);
    const label = `continental-${site.journey}-composed`;
    const receiptPath = `/tmp/mclone-world-explorer-web-desktop-${label}-receipt.json`;
    browserReceipts.push(JSON.parse(await readFile(receiptPath)));
  }
}

const direct = JSON.parse(await readFile(directReceipt));
const receipt = {
  schemaRevision: "mclone-continental-exact-review-package-v1",
  sourceCommit: commandText("git", ["rev-parse", "HEAD"]),
  gitDirty: commandText("git", ["status", "--porcelain"]).length > 0,
  exactRadius,
  direct,
  nativeCaptures,
  nativeSmokes,
  browserReceipts,
};
const receiptPath = path.join(outputRoot, "review-index.json");
await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ status: "ok", receipt: receiptPath }, null, 2));

function reviewSiteArgs(site) {
  const result = ["--source", "continental", "--journey", site.journey];
  if (site.centerX !== undefined) {
    result.push("--center-x", site.centerX, "--center-z", site.centerZ);
  }
  return result;
}

async function artifact(file) {
  return { file, bytes: (await stat(file)).size };
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

function commandText(command, args) {
  const result = spawnSync(command, args, {
    cwd: repositoryRoot,
    env: process.env,
    encoding: "utf8",
  });
  if (result.error || result.status !== 0) {
    return "unknown";
  }
  return result.stdout.trim();
}
