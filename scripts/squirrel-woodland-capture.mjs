import { spawnSync } from "node:child_process";
import { existsSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(fileURLToPath(new URL("..", import.meta.url)));
const root = "/tmp/mclone-squirrel-woodland-fixture";
const image = "/tmp/mclone-squirrel-woodland.png";
const stereoImage = "/tmp/mclone-squirrel-woodland-stereo.png";

const fixtureOutput = run([
  "cargo", "run", "--manifest-path", "native/Cargo.toml", "-p", "mclone-server",
  "--bin", "playable_showcase_fixture", "--", "--showcase", "squirrel-woodland",
  "--root", root,
]);
const receipt = Object.fromEntries(
  fixtureOutput
    .split("\n")
    .find((line) => line.startsWith("MCLONE_PLAYABLE_SHOWCASE_FIXTURE "))
    ?.slice("MCLONE_PLAYABLE_SHOWCASE_FIXTURE ".length)
    .split(" ")
    .map((field) => field.split("=", 2)) ?? [],
);
if (receipt.id !== "squirrel-woodland" || receipt.revision !== "1") {
  throw new Error("showcase compiler did not emit the expected squirrel receipt");
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
  "--screenshot-hud", "false",
  "--screenshot-eye", receipt.entry_eye,
  "--screenshot-target", receipt.entry_target,
], env);
if (!/3 entities, 3 actors, [1-3] drawn actors/.test(output)) {
  throw new Error("squirrel capture did not draw an ordinary runtime actor");
}
if (!existsSync(image) || statSync(image).size < 10_000) {
  throw new Error(`squirrel capture did not write credible pixels to ${image}`);
}

const stereoOutput = run([
  "cargo", "run", "--manifest-path", "native/Cargo.toml", "-p", "mclone-native-client",
  "--bin", "mclone-native-client", "--",
  "--xr-emulation-screenshot", stereoImage, "--width", "960", "--height", "720",
  "--xr-emulation-pause-panel", "false", ...common,
], env);
const eyeDifferences = Number(
  stereoOutput.match(/([0-9]+) differing eye pixels/)?.[1] ?? 0,
);
if (eyeDifferences < 1_000) {
  throw new Error("squirrel stereo capture did not produce distinct ordinary eye views");
}
if (!existsSync(stereoImage) || statSync(stereoImage).size < 10_000) {
  throw new Error(`squirrel stereo capture did not write credible pixels to ${stereoImage}`);
}
console.log(
  `MCLONE_SQUIRREL_WOODLAND_CAPTURE id=${receipt.id} revision=${receipt.revision} seed=${receipt.seed} entry_eye=${receipt.entry_eye} entry_target=${receipt.entry_target} entity_count=${receipt.entity_count} flat=${image} stereo=${stereoImage}`,
);

function run(command, environment = process.env) {
  const result = spawnSync(command[0], command.slice(1), {
    cwd: repo, env: environment, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  if (result.status !== 0) throw new Error(`${command.join(" ")} exited ${result.status}`);
  return `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
}
