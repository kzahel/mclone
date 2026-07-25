import { spawnSync } from "node:child_process";
import { cp, mkdir, rm, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptRoot = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(scriptRoot, "..");
const nativeRoot = path.resolve(appRoot, "../..");
const repositoryRoot = path.resolve(nativeRoot, "..");
const wasmBindgenVersion = "0.2.125";
const bindgenRoot = path.join(
  nativeRoot,
  "target",
  `wasm-bindgen-cli-${wasmBindgenVersion}`,
);
const bindgen = path.join(bindgenRoot, "bin", "wasm-bindgen");
const wasm = path.join(
  nativeRoot,
  "target",
  "wasm32-unknown-unknown",
  "release",
  "mclone_world_explorer.wasm",
);
const output = path.join(nativeRoot, "target", "mclone-world-explorer-www");
const pkg = path.join(output, "pkg");
const firstPartyPacks = path.join(
  repositoryRoot,
  "generated-assets",
  "first-party-stage",
  "first-party-packs",
);

run("cargo", [
  "build",
  "--manifest-path",
  path.join(nativeRoot, "Cargo.toml"),
  "-p",
  "mclone-world-explorer",
  "--lib",
  "--release",
  "--target",
  "wasm32-unknown-unknown",
], repositoryRoot);

if (!(await exists(bindgen))) {
  run("cargo", [
    "install",
    "wasm-bindgen-cli",
    "--version",
    wasmBindgenVersion,
    "--locked",
    "--root",
    bindgenRoot,
  ], repositoryRoot);
}

await rm(output, { recursive: true, force: true });
await cp(path.join(appRoot, "www"), output, { recursive: true });
await mkdir(pkg, { recursive: true });
await cp(firstPartyPacks, path.join(output, "first-party-packs"), {
  recursive: true,
});
run(bindgen, [
  "--target",
  "web",
  "--typescript",
  "--out-dir",
  pkg,
  "--out-name",
  "mclone_world_explorer",
  wasm,
], repositoryRoot);

const wasmBytes = (await stat(
  path.join(pkg, "mclone_world_explorer_bg.wasm"),
)).size;
console.log(`World Explorer web root: ${output}`);
console.log(`World Explorer Wasm: ${wasmBytes} bytes`);

async function exists(file) {
  try {
    await stat(file);
    return true;
  } catch {
    return false;
  }
}

function run(command, args, cwd) {
  const result = spawnSync(command, args, {
    cwd,
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
