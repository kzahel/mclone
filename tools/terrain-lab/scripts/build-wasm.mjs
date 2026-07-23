import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const terrainLabRoot = path.resolve(scriptDir, "..");
const repositoryRoot = path.resolve(terrainLabRoot, "..", "..");
const nativeRoot = path.join(repositoryRoot, "native");
const wasmBindgenVersion = "0.2.125";
const wasmPath = path.join(
  nativeRoot,
  "target",
  "wasm32-unknown-unknown",
  "debug",
  "mclone_terrain_lab.wasm",
);
const bindgenRoot = path.join(
  nativeRoot,
  "target",
  `wasm-bindgen-cli-${wasmBindgenVersion}`,
);
const bindgenBin = path.join(
  bindgenRoot,
  "bin",
  process.platform === "win32" ? "wasm-bindgen.exe" : "wasm-bindgen",
);
const outputDir = path.join(terrainLabRoot, "generated", "pkg");

run(
  "cargo",
  [
    "build",
    "--manifest-path",
    path.join(nativeRoot, "Cargo.toml"),
    "-p",
    "mclone-terrain-lab",
    "--target",
    "wasm32-unknown-unknown",
  ],
  repositoryRoot,
);

if (!existsSync(bindgenBin)) {
  run(
    "cargo",
    [
      "install",
      "wasm-bindgen-cli",
      "--version",
      wasmBindgenVersion,
      "--locked",
      "--root",
      bindgenRoot,
    ],
    repositoryRoot,
  );
}

rmSync(outputDir, { force: true, recursive: true });
mkdirSync(outputDir, { recursive: true });
run(
  bindgenBin,
  [
    "--target",
    "web",
    "--typescript",
    "--out-dir",
    outputDir,
    "--out-name",
    "mclone_terrain_lab",
    wasmPath,
  ],
  repositoryRoot,
);

console.log(`Terrain Lab WASM bindings ready: ${outputDir}`);

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
