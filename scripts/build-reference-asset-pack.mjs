#!/usr/bin/env node
import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_DIR = path.resolve(SCRIPT_DIR, "..");
const VERSION = process.argv[2] ?? "1.17.1";
const REFERENCE_DIR = path.join(PROJECT_DIR, "reference", `minecraft-${VERSION}`);
const ASSET_DIR = path.join(REFERENCE_DIR, "extracted");
const ZIP_PATH = path.join(REFERENCE_DIR, "extracted.zip");
const MANIFEST_PATH = `${ZIP_PATH}.json`;

async function collectFiles(directory, baseDirectory = directory) {
  const entries = await fs.readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...await collectFiles(absolutePath, baseDirectory));
    } else if (entry.isFile()) {
      files.push(path.relative(baseDirectory, absolutePath).split(path.sep).join("/"));
    }
  }

  return files.sort((left, right) => left.localeCompare(right));
}

async function assertAssetTree() {
  const stats = await fs.stat(ASSET_DIR).catch(() => undefined);
  if (!stats?.isDirectory()) {
    throw new Error(`${ASSET_DIR} not found. Run scripts/extract-assets.sh ${VERSION} first.`);
  }

  const assetsStats = await fs.stat(path.join(ASSET_DIR, "assets")).catch(() => undefined);
  if (!assetsStats?.isDirectory()) {
    throw new Error(`${ASSET_DIR}/assets not found. Re-run scripts/extract-assets.sh ${VERSION}.`);
  }
}

async function runZip(files, outputPath) {
  await fs.rm(outputPath, { force: true });
  await fs.mkdir(path.dirname(outputPath), { recursive: true });

  await new Promise((resolve, reject) => {
    const zip = spawn("zip", ["-0", "-X", "-q", outputPath, "-@"], {
      cwd: ASSET_DIR,
      stdio: ["pipe", "inherit", "inherit"],
    });
    zip.on("error", reject);
    zip.on("close", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`zip exited with code ${code}`));
      }
    });
    zip.stdin.end(`${files.join("\n")}\n`);
  });
}

async function sha256File(filePath) {
  const hash = createHash("sha256");
  await new Promise((resolve, reject) => {
    const stream = createReadStream(filePath);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("error", reject);
    stream.on("end", resolve);
  });
  return hash.digest("hex");
}

await assertAssetTree();
const files = await collectFiles(ASSET_DIR);
if (files.length === 0) {
  throw new Error(`${ASSET_DIR} did not contain any files`);
}

const tempZipPath = path.join(REFERENCE_DIR, `.extracted.${process.pid.toString()}.zip.tmp`);
try {
  await runZip(files, tempZipPath);
  const [hash, stats] = await Promise.all([
    sha256File(tempZipPath),
    fs.stat(tempZipPath),
  ]);
  await fs.rename(tempZipPath, ZIP_PATH);
  await fs.writeFile(
    MANIFEST_PATH,
    `${JSON.stringify({
      version: VERSION,
      zip: "extracted.zip",
      sha256: hash,
      size: stats.size,
      fileCount: files.length,
      generatedAt: new Date().toISOString(),
    }, null, 2)}\n`,
  );
  console.log(`Wrote ${path.relative(PROJECT_DIR, ZIP_PATH)} (${stats.size.toString()} bytes, ${files.length.toString()} files)`);
  console.log(`Wrote ${path.relative(PROJECT_DIR, MANIFEST_PATH)} (sha256 ${hash})`);
} finally {
  await fs.rm(tempZipPath, { force: true });
}
