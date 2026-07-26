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
const nativePackages = cargoTree(process.env.WORLD_EXPLORER_NATIVE_TARGET
  ?? "x86_64-unknown-linux-gnu");
const browserPackages = cargoTree("wasm32-unknown-unknown");

const allowedMclonePackages = new Set([
  "mclone-assets",
  "mclone-core",
  "mclone-light",
  "mclone-mesh",
  "mclone-render-color",
  "mclone-terrain-view",
  "mclone-view-control",
  "mclone-world-explorer",
  "mclone-worldgen",
]);
const forbiddenPackages = new Set([
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
]);

const unexpectedMclonePackages = [...new Set([
  ...nativePackages,
  ...browserPackages,
])]
  .filter((name) =>
    name.startsWith("mclone-") && !allowedMclonePackages.has(name))
  .sort();
const forbiddenFound = [...new Set([
  ...nativePackages,
  ...browserPackages,
])]
  .filter((name) => forbiddenPackages.has(name))
  .sort();
const browserOnlyPackages = new Set([
  "js-sys",
  "wasm-bindgen",
  "wasm-bindgen-futures",
  "web-sys",
]);
const browserPackagesInNativeGraph = [...nativePackages]
  .filter((name) => browserOnlyPackages.has(name))
  .sort();
const nativeOnlyPackages = new Set([
  "env_logger",
  "pollster",
  "winit",
]);
const nativePackagesInBrowserGraph = [...browserPackages]
  .filter((name) => nativeOnlyPackages.has(name))
  .sort();

if (
  unexpectedMclonePackages.length
  || forbiddenFound.length
  || browserPackagesInNativeGraph.length
  || nativePackagesInBrowserGraph.length
) {
  console.error("World Explorer dependency firewall failed.");
  if (unexpectedMclonePackages.length) {
    console.error(
      `Unexpected Mclone packages: ${unexpectedMclonePackages.join(", ")}`,
    );
  }
  if (forbiddenFound.length) {
    console.error(`Forbidden packages: ${forbiddenFound.join(", ")}`);
  }
  if (browserPackagesInNativeGraph.length) {
    console.error(
      `Browser packages leaked into native: ${browserPackagesInNativeGraph.join(", ")}`,
    );
  }
  if (nativePackagesInBrowserGraph.length) {
    console.error(
      `Native packages leaked into browser: ${nativePackagesInBrowserGraph.join(", ")}`,
    );
  }
  process.exit(1);
}

console.log(
  "World Explorer dependency firewall passed "
    + `(${nativePackages.size} native, ${browserPackages.size} browser packages).`,
);

function cargoTree(target) {
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
      "--target",
      target,
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
  return new Set(
    result.stdout
      .split(/\r?\n/u)
      .map((line) => line.trim().split(/\s+/u)[0])
      .filter(Boolean),
  );
}
