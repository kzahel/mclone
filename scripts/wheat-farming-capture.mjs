import { spawnSync } from "node:child_process";
import { existsSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(fileURLToPath(new URL("..", import.meta.url)));
const root = "/tmp/mclone-wheat-farming-fixture";
const image = "/tmp/mclone-wheat-farming.png";

const fixtureOutput = run([
  "cargo", "run", "--manifest-path", "native/Cargo.toml", "-p", "mclone-server",
  "--bin", "wheat_farming_fixture", "--", "--root", root,
]);
const receipt = Object.fromEntries(
  fixtureOutput
    .split("\n")
    .find((line) => line.startsWith("MCLONE_PLAYABLE_SHOWCASE_FIXTURE "))
    ?.slice("MCLONE_PLAYABLE_SHOWCASE_FIXTURE ".length)
    .split(" ")
    .map((field) => field.split("=", 2)) ?? [],
);
if (receipt.id !== "wheat-farming" || receipt.revision !== "2") {
  throw new Error("showcase compiler did not emit the expected wheat receipt");
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
if (!/GUI commands/.test(output)) {
  throw new Error("wheat capture did not include the shared HUD");
}
if (!existsSync(image) || statSync(image).size < 10_000) {
  throw new Error(`wheat capture did not write credible pixels to ${image}`);
}
console.log(
  `MCLONE_WHEAT_FARMING_CAPTURE id=${receipt.id} revision=${receipt.revision} seed=${receipt.seed} entry_eye=${receipt.entry_eye} entry_target=${receipt.entry_target} flat=${image}`,
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
