#!/usr/bin/env node

import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import { createWriteStream } from "node:fs";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

function parseArgs(argv) {
  const result = {
    matrix: "test/fixtures/creatures/wildlife-population-matrix-v1.json",
    output: null,
    jobs: 2,
    skipBuild: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    if (flag === "--help" || flag === "-h") {
      process.stdout.write(
        "run-wildlife-population-campaign.mjs --output <empty-dir> "
          + "[--matrix <json>] [--jobs <n>] [--skip-build]\n",
      );
      process.exit(0);
    }
    if (flag === "--skip-build") {
      result.skipBuild = true;
      continue;
    }
    const value = argv[index + 1];
    if (!value) throw new Error(`missing value for ${flag}`);
    if (flag === "--matrix") result.matrix = value;
    else if (flag === "--output") result.output = value;
    else if (flag === "--jobs") result.jobs = Number.parseInt(value, 10);
    else throw new Error(`unknown argument ${flag}`);
    index += 1;
  }
  if (!result.output) throw new Error("--output is required");
  if (!Number.isInteger(result.jobs) || result.jobs < 1 || result.jobs > 8) {
    throw new Error("--jobs must be an integer in 1..=8");
  }
  return result;
}

function runCommand(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd ?? process.cwd(),
      stdio: options.stdio ?? "inherit",
      env: process.env,
    });
    child.once("error", reject);
    child.once("exit", (code, signal) => {
      if (code === 0) resolve();
      else reject(new Error(`${command} exited ${code ?? signal}`));
    });
  });
}

async function ensureEmptyDirectory(directory) {
  try {
    const entries = await readdir(directory);
    if (entries.length > 0) throw new Error(`output directory ${directory} is not empty`);
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
    await mkdir(directory, { recursive: true });
  }
}

async function runOne(binary, root, matrix, entry) {
  const directory = path.join(root, entry.label);
  const command = [
    "--seed", String(entry.seed),
    "--center", `${entry.center[0]},${entry.center[1]}`,
    "--radius", String(entry.radius),
    "--days", String(entry.days),
    "--canary-ticks", String(matrix.canaryTicks ?? 480),
    "--output", directory,
    "--label", entry.label,
  ];
  process.stderr.write(`[wildlife:${entry.label}] starting ${entry.days} days\n`);
  const started = performance.now();
  const log = createWriteStream(path.join(root, `${entry.label}.log`));
  try {
    await new Promise((resolve, reject) => {
      log.once("open", resolve);
      log.once("error", reject);
    });
    await runCommand(binary, command, { stdio: ["ignore", log, log] });
    const manifest = JSON.parse(await readFile(path.join(directory, "manifest.json"), "utf8"));
    const timing = JSON.parse(await readFile(path.join(directory, "performance.json"), "utf8"));
    const daily = (await readFile(path.join(directory, "daily.csv"), "utf8"))
      .trim().split("\n");
    const header = daily[0].split(",");
    const values = daily.at(-1).split(",");
    const final = Object.fromEntries(header.map((name, index) => [name, values[index]]));
    process.stderr.write(
      `[wildlife:${entry.label}] pass in ${((performance.now() - started) / 1000).toFixed(1)}s; `
        + `rabbits=${final.rabbits} deer=${final.deer}\n`,
    );
    return { label: entry.label, status: "passed", directory: entry.label, manifest, timing, final };
  } catch (error) {
    process.stderr.write(`[wildlife:${entry.label}] FAILED: ${error.message}\n`);
    return { label: entry.label, status: "failed", directory: entry.label, error: error.message };
  } finally {
    log.end();
  }
}

async function runPool(items, jobs, worker) {
  const results = new Array(items.length);
  let next = 0;
  await Promise.all(Array.from({ length: Math.min(jobs, items.length) }, async () => {
    while (next < items.length) {
      const index = next;
      next += 1;
      results[index] = await worker(items[index]);
    }
  }));
  return results;
}

function htmlEscape(value) {
  return String(value).replaceAll("&", "&amp;").replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

function campaignHtml(campaign) {
  const rows = campaign.results.map((result) => {
    if (result.status !== "passed") {
      return `<tr class="fail"><td>${htmlEscape(result.label)}</td><td>FAIL</td><td colspan="6">${htmlEscape(result.error)}</td></tr>`;
    }
    return `<tr><td><a href="${encodeURIComponent(result.directory)}/report.html">${htmlEscape(result.label)}</a></td><td>pass</td><td>${result.manifest.durationDays}</td><td>${result.final.rabbits}</td><td>${result.final.deer}</td><td>${result.timing.peakLiving}</td><td>${result.timing.runMillis}</td><td><code>${result.manifest.dailySeriesChecksum.slice(0, 12)}</code></td></tr>`;
  }).join("\n");
  return `<!doctype html><meta charset="utf-8"><title>Mclone wildlife campaign</title><style>body{font:16px system-ui;background:#111712;color:#edf4e8;max-width:1200px;margin:40px auto;padding:0 20px}table{width:100%;border-collapse:collapse;background:#182219}th,td{padding:10px;border:1px solid #344638;text-align:right}th:first-child,td:first-child{text-align:left}a{color:#9fdab0}.fail{background:#482522}code{color:#d8e7a9}</style><h1>Wildlife population campaign</h1><p>Matrix checksum <code>${campaign.matrixChecksum}</code>. Overall: <b>${campaign.allPassed ? "PASS" : "FAIL"}</b>.</p><table><thead><tr><th>Window</th><th>Status</th><th>Days</th><th>Rabbits</th><th>Deer</th><th>Peak</th><th>Runtime ms</th><th>Receipt</th></tr></thead><tbody>${rows}</tbody></table>`;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const root = path.resolve(args.output);
  await ensureEmptyDirectory(root);
  const matrixBytes = await readFile(path.resolve(args.matrix));
  const matrix = JSON.parse(matrixBytes.toString("utf8"));
  if (matrix.schemaVersion !== 1 || !Array.isArray(matrix.runs) || matrix.runs.length === 0) {
    throw new Error("unsupported or empty campaign matrix");
  }
  if (!args.skipBuild) {
    await runCommand("cargo", [
      "build", "--release", "--manifest-path", "native/Cargo.toml",
      "-p", "mclone-server", "--bin", "wildlife_population_sim",
    ]);
  }
  const binary = path.resolve("native/target/release/wildlife_population_sim");
  const results = await runPool(matrix.runs, args.jobs, (entry) => (
    runOne(binary, root, matrix, entry)
  ));
  const campaign = {
    schemaVersion: 1,
    matrixPath: args.matrix,
    matrixChecksum: createHash("sha256").update(matrixBytes).digest("hex"),
    jobs: args.jobs,
    allPassed: results.every((result) => result.status === "passed"),
    results,
  };
  await writeFile(path.join(root, "campaign.json"), JSON.stringify(campaign, null, 2));
  await writeFile(path.join(root, "index.html"), campaignHtml(campaign));
  process.stdout.write(`campaign report: ${path.join(root, "index.html")}\n`);
  if (!campaign.allPassed) process.exitCode = 1;
}

main().catch((error) => {
  process.stderr.write(`wildlife campaign failed: ${error.stack ?? error.message}\n`);
  process.exitCode = 1;
});
