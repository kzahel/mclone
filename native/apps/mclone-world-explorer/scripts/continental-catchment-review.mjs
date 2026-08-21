import { spawnSync } from "node:child_process";
import { copyFile, mkdir, readFile, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptRoot = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(scriptRoot, "..");
const nativeRoot = path.resolve(appRoot, "../..");
const repositoryRoot = path.resolve(nativeRoot, "..");
const targetRoot = path.join(nativeRoot, "target", "debug");
const explorerBinary = path.join(targetRoot, "mclone-world-explorer");
const outputRoot = argumentValue("--output")
  ?? "/tmp/mclone-continental-catchment-review";
const skipBuild = process.argv.includes("--skip-build");
const skipBrowser = process.argv.includes("--skip-browser");
const skipMovement = process.argv.includes("--skip-movement");
const exactRadius = Number.parseInt(argumentValue("--exact-radius") ?? "5", 10);
const browserEnvironment = { ...process.env, HEADED: "1" };

await mkdir(outputRoot, { recursive: true });
if (!skipBuild) {
  run("cargo", [
    "build",
    "--manifest-path", path.join(nativeRoot, "Cargo.toml"),
    "-p", "mclone-worldgen",
    "--bin", "mclone_continental_catchment_review",
    "--bin", "mclone_continental_catchment_exact_review",
    "--bin", "mclone_continental_catchment_perf",
  ]);
  run("cargo", [
    "build",
    "--manifest-path", path.join(nativeRoot, "Cargo.toml"),
    "-p", "mclone-world-explorer",
    "--bin", "mclone-world-explorer",
  ]);
}

const catalogPath = path.join(outputRoot, "catalog.json");
run(path.join(targetRoot, "mclone_continental_catchment_review"), [
  "--output", catalogPath,
]);
const catalog = JSON.parse(await readFile(catalogPath));

const directExactPath = path.join(outputRoot, "direct-exact.json");
run(path.join(targetRoot, "mclone_continental_catchment_exact_review"), [
  "--output", directExactPath,
]);
const directExact = JSON.parse(await readFile(directExactPath));

const performancePath = path.join(outputRoot, "performance.json");
run(path.join(targetRoot, "mclone_continental_catchment_perf"), [
  "--output", performancePath,
]);
const performance = JSON.parse(await readFile(performancePath));

const nativeSites = [];
for (const site of catalog.sites) {
  const captures = [];
  for (const frame of nativeFrames(site)) {
    const capture = path.join(outputRoot, `${site.kind}-${frame.label}.png`);
    const elapsedMs = timedRun(explorerBinary, [
      "--source", "continental",
      "--catchment-site", site.kind,
      "--view", "3d",
      "--blocks-across", frame.blocksAcross.toString(),
      "--composition", frame.composition,
      "--exact-radius", exactRadius.toString(),
      "--capture", capture,
    ]);
    captures.push({
      label: frame.label,
      composition: frame.composition,
      blocksAcross: frame.blocksAcross,
      elapsedMs,
      ...await artifact(capture),
    });
  }

  let movement = null;
  if (!skipMovement) {
    const smokeRoot = path.join(outputRoot, `${site.kind}-movement`);
    const elapsedMs = timedRun(explorerBinary, [
      "--source", "continental",
      "--catchment-site", site.kind,
      "--view", "3d",
      "--blocks-across", "512",
      "--composition", "composed",
      "--exact-radius", exactRadius.toString(),
      "--retained-smoke", smokeRoot,
    ]);
    movement = {
      elapsedMs,
      window: JSON.parse(await readFile(path.join(smokeRoot, "window", "receipt.json"))),
      offscreen: JSON.parse(await readFile(path.join(smokeRoot, "offscreen", "receipt.json"))),
    };
  }
  nativeSites.push({ kind: site.kind, captures, movement });
}

const browserSites = [];
if (!skipBrowser) {
  if (!skipBuild) {
    run("node", [path.join(scriptRoot, "build-web.mjs")]);
  }
  for (const site of catalog.sites) {
    const browserArgs = [
      path.join(scriptRoot, "browser-composition-smoke.mjs"),
      "--skip-build",
      "--source", "continental",
      "--catchment-site", site.kind,
      "--composition", "composed",
      "--blocks-across", site.reviewFrames.obliqueBlocks.toString(),
      "--exact-radius", exactRadius.toString(),
      "--yaw", site.reviewFrames.yawRadians.toString(),
      "--pitch", site.reviewFrames.pitchRadians.toString(),
    ];
    const elapsedMs = timedRun("node", browserArgs, browserEnvironment);
    const label = `continental-${site.kind}-composed`;
    const sourceReceipt =
      `/tmp/mclone-world-explorer-web-desktop-${label}-receipt.json`;
    const receipt = JSON.parse(await readFile(sourceReceipt));
    const receiptPath = path.join(outputRoot, `${site.kind}-browser-receipt.json`);
    await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
    let capture = null;
    if (receipt.capture) {
      capture = path.join(outputRoot, `${site.kind}-browser.png`);
      await copyFile(receipt.capture, capture);
    }
    browserSites.push({
      kind: site.kind,
      elapsedMs,
      receipt: await artifact(receiptPath),
      capture: capture ? await artifact(capture) : null,
      report: receipt.report,
      url: receipt.url,
    });
  }
}

const receipt = {
  schemaRevision: "mclone-continental-catchment-review-package-v1",
  sourceCommit: commandText("git", ["rev-parse", "HEAD"]),
  gitDirty: commandText("git", ["status", "--porcelain"]).length > 0,
  exactRadius,
  catalog,
  directExact,
  performance,
  nativeSites,
  browserSites,
};
const receiptPath = path.join(outputRoot, "review-index.json");
await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ status: "ok", receipt: receiptPath }, null, 2));

function nativeFrames(site) {
  return [
    {
      label: "exact",
      composition: "exact",
      blocksAcross: site.reviewFrames.exactBlocks,
    },
    { label: "coverage", composition: "coverage", blocksAcross: 512 },
    {
      label: "composed",
      composition: "composed",
      blocksAcross: site.reviewFrames.obliqueBlocks,
    },
    {
      label: "horizon",
      composition: "horizon",
      blocksAcross: site.reviewFrames.overviewBlocks,
    },
  ];
}

async function artifact(file) {
  return { file, bytes: (await stat(file)).size };
}

function timedRun(command, args, environment = process.env) {
  const started = process.hrtime.bigint();
  run(command, args, environment);
  return Number(process.hrtime.bigint() - started) / 1_000_000;
}

function run(command, args, environment = process.env) {
  const result = spawnSync(command, args, {
    cwd: repositoryRoot,
    env: environment,
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

function argumentValue(name) {
  const inline = process.argv.find((argument) => argument.startsWith(`${name}=`));
  if (inline) {
    return inline.slice(name.length + 1);
  }
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}
