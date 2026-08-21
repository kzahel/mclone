import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptRoot = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(scriptRoot, "../../../..");
const nativeRoot = path.join(repositoryRoot, "native");
const binary = path.join(nativeRoot, "target", "debug", "mclone-world-explorer");
const journeys = [
  "coast-to-wooded-interior",
  "clearing-between-forest-cores",
  "long-forest-edge",
  "connected-water-country",
  "quiet-rolling-interior",
  "mesa-desert",
];
const frames = [
  ["locator", "map", "65536"],
  ["overview", "map", "16384"],
  ["oblique", "3d", "8192"],
  ["habitat", "3d", "512"],
];

run("cargo", [
  "build",
  "--manifest-path",
  path.join(nativeRoot, "Cargo.toml"),
  "-p",
  "mclone-world-explorer",
]);

const captures = [];
for (const journey of journeys) {
  for (const [frame, view, blocks] of frames) {
    const capture = `/tmp/mclone-journey-${journey}-${frame}-native.png`;
    run(binary, [
      "--journey", journey,
      "--view", view,
      "--blocks-across", blocks,
      "--capture", capture,
    ]);
    captures.push(capture);
  }
}

console.log(JSON.stringify({ status: "ok", captures }, null, 2));

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
