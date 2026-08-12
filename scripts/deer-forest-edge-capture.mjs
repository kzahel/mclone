import { spawnSync } from "node:child_process";
import { existsSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(fileURLToPath(new URL("..", import.meta.url)));
const root = "/tmp/mclone-deer-forest-edge-fixture";
const image = "/tmp/mclone-deer-forest-edge.png";
const stereoImage = "/tmp/mclone-deer-forest-edge-stereo.png";

const fixtureOutput = run([
  "cargo", "run", "--manifest-path", "native/Cargo.toml", "-p", "mclone-server",
  "--bin", "deer_forest_edge_fixture", "--", "--root", root,
]);
const receipt = Object.fromEntries(
  fixtureOutput
    .split("\n")
    .find((line) => line.startsWith("MCLONE_PLAYABLE_SHOWCASE_FIXTURE "))
    ?.slice("MCLONE_PLAYABLE_SHOWCASE_FIXTURE ".length)
    .split(" ")
    .map((field) => field.split("=", 2)) ?? [],
);
if (receipt.id !== "deer-forest-edge" || receipt.revision !== "1") {
  throw new Error("showcase compiler did not emit the expected deer receipt");
}

const common = [
  "--world-dir", `${root}/world`,
  "--generation-profile", "authored-only",
  "--seed", receipt.seed,
  "--chunk-x", "0", "--chunk-z", "0", "--render-distance", "2",
  "--day-time", receipt.day_time,
  ...(receipt.freeze_time === "true" ? ["--freeze-time"] : []),
  "--debug-passive-showcase", "false",
  "--lighting", "false", "--fullbright", "true",
];
const env = { ...process.env, MCLONE_PLAYER_PROFILE_FILE: `${root}/player-profile.v1.json` };
const output = run([
  "cargo", "run", "--manifest-path", "native/Cargo.toml", "-p", "mclone-native-client",
  "--bin", "mclone-native-client", "--",
  "--screenshot", image, "--width", "1600", "--height", "900",
  ...common,
  "--screenshot-hud", "true",
  "--screenshot-eye", receipt.entry_eye,
  "--screenshot-target", receipt.entry_target,
], env);
if (!/4 entities, 4 actors, 4 drawn actors/.test(output)) {
  throw new Error("deer capture did not draw three deer and one bed sign");
}
if (!/GUI commands/.test(output)) {
  throw new Error("deer capture did not include the HUD/field-guide draw list");
}
if (!existsSync(image) || statSync(image).size < 10_000) {
  throw new Error(`deer capture did not write credible pixels to ${image}`);
}

const stereoOutput = run([
  "cargo", "run", "--manifest-path", "native/Cargo.toml", "-p", "mclone-native-client",
  "--bin", "mclone-native-client", "--",
  "--xr-emulation-screenshot", stereoImage, "--width", "960", "--height", "720",
  "--xr-emulation-pause-panel", "false", ...common,
], env);
if (!/2 eye UI composites/.test(stereoOutput)) {
  throw new Error("deer stereo capture did not composite field notes into both eyes");
}
if (!existsSync(stereoImage) || statSync(stereoImage).size < 10_000) {
  throw new Error(`deer stereo capture did not write credible pixels to ${stereoImage}`);
}
console.log(
  `MCLONE_DEER_FOREST_EDGE_CAPTURE id=${receipt.id} revision=${receipt.revision} seed=${receipt.seed} entry_eye=${receipt.entry_eye} entry_target=${receipt.entry_target} entity_count=${receipt.entity_count} deer_count=${receipt.deer_count} deer_bed_count=${receipt.deer_bed_count} deer_field_guide_bits=${receipt.deer_field_guide_bits} flat=${image} stereo=${stereoImage}`,
);

function run(command, env = process.env) {
  const result = spawnSync(command[0], command.slice(1), {
    cwd: repo, env, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  if (result.status !== 0) throw new Error(`${command.join(" ")} exited ${result.status}`);
  return `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
}
