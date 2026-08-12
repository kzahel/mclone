import { spawnSync } from "node:child_process";
import { existsSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(fileURLToPath(new URL("..", import.meta.url)));
const root = "/tmp/mclone-mallard-ecology-fixture";
const image = "/tmp/mclone-mallard-ecology.png";

run([
  "cargo", "run", "--manifest-path", "native/Cargo.toml", "-p", "mclone-server",
  "--bin", "mallard_ecology_fixture", "--", "--root", root,
]);
const output = run([
  "cargo", "run", "--manifest-path", "native/Cargo.toml", "-p", "mclone-native-client",
  "--bin", "mclone-native-client", "--",
  "--screenshot", image,
  "--width", "1600",
  "--height", "900",
  "--world-dir", `${root}/world`,
  "--generation-profile", "authored-only",
  "--seed", "17502",
  "--chunk-x", "0",
  "--chunk-z", "0",
  "--render-distance", "2",
  "--day-time", "6000",
  "--freeze-time",
  "--debug-passive-showcase", "false",
  "--screenshot-hud", "true",
  "--screenshot-eye", "10,72,19",
  "--screenshot-target", "3,64.5,8",
  "--lighting", "false",
  "--fullbright", "true",
], {
  ...process.env,
  MCLONE_PLAYER_PROFILE_FILE: `${root}/player-profile.v1.json`,
});

if (!/4 entities, 5 actors, 5 drawn actors/.test(output)) {
  throw new Error("mallard ecology capture did not draw four entities as five figure instances");
}
if (!/220 GUI commands/.test(output)) {
  throw new Error("mallard ecology capture did not include the full HUD/field-guide draw list");
}
if (!existsSync(image) || statSync(image).size < 10_000) {
  throw new Error(`mallard ecology capture did not write credible pixels to ${image}`);
}
console.log(`MCLONE_MALLARD_ECOLOGY_CAPTURE image=${image}`);

function run(command, env = process.env) {
  const result = spawnSync(command[0], command.slice(1), {
    cwd: repo,
    env,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  if (result.status !== 0) {
    throw new Error(`${command.join(" ")} exited ${result.status}`);
  }
  return `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
}
