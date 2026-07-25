#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const manifestPath = path.join(repoRoot, "native", "Cargo.toml");
const cargo = process.env.CARGO || "cargo";
const result = spawnSync(
  cargo,
  [
    "tree",
    "--manifest-path",
    manifestPath,
    "-p",
    "mclone-world-explorer",
    "-e",
    "normal",
    "--prefix",
    "none",
    "--format",
    "{p}",
  ],
  {
    cwd: repoRoot,
    encoding: "utf8",
  },
);

if (result.error) {
  console.error(`failed to launch ${cargo}: ${result.error.message}`);
  process.exit(1);
}
if (result.status !== 0) {
  process.stderr.write(result.stderr);
  process.exit(result.status ?? 1);
}

const packages = new Set(
  result.stdout
    .split(/\r?\n/u)
    .map((line) => line.trim().split(/\s+/u)[0])
    .filter(Boolean),
);

const allowedMclonePackages = new Set([
  "mclone-assets",
  "mclone-core",
  "mclone-light",
  "mclone-mesh",
  "mclone-terrain-view",
  "mclone-view-control",
  "mclone-world-explorer",
  "mclone-worldgen",
]);
const forbiddenPackages = new Set([
  "js-sys",
  "mclone-app-runtime",
  "mclone-audio",
  "mclone-client",
  "mclone-native-client",
  "mclone-net",
  "mclone-physics",
  "mclone-protocol",
  "mclone-scene",
  "mclone-server",
  "mclone-ui",
  "mclone-web-client",
  "mclone-xr-graphics",
  "mclone-xr-host",
  "openxr",
  "wasm-bindgen",
  "web-sys",
]);

const unexpectedMclonePackages = [...packages]
  .filter(
    (name) =>
      name.startsWith("mclone-") && !allowedMclonePackages.has(name),
  )
  .sort();
const forbiddenFound = [...packages]
  .filter((name) => forbiddenPackages.has(name))
  .sort();

if (unexpectedMclonePackages.length || forbiddenFound.length) {
  console.error("World Explorer dependency firewall failed.");
  if (unexpectedMclonePackages.length) {
    console.error(
      `Unexpected Mclone packages: ${unexpectedMclonePackages.join(", ")}`,
    );
  }
  if (forbiddenFound.length) {
    console.error(`Forbidden packages: ${forbiddenFound.join(", ")}`);
  }
  process.exit(1);
}

console.log(
  `World Explorer dependency firewall passed (${packages.size} packages).`,
);
