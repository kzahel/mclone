#!/usr/bin/env node

import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const output = process.argv[2]
  ? path.resolve(process.argv[2])
  : path.join(tmpdir(), "mclone-world-explorer-smoke");
const environment = { ...process.env };

if (
  process.platform === "linux"
  && !environment.WAYLAND_DISPLAY
  && environment.XDG_RUNTIME_DIR
) {
  const waylandSocket = path.join(environment.XDG_RUNTIME_DIR, "wayland-0");
  if (existsSync(waylandSocket)) {
    environment.WAYLAND_DISPLAY = "wayland-0";
  }
}

const result = spawnSync(
  environment.CARGO || "cargo",
  [
    "run",
    "--release",
    "--manifest-path",
    "native/Cargo.toml",
    "-p",
    "mclone-world-explorer",
    "--",
    "--smoke",
    output,
  ],
  {
    cwd: repoRoot,
    env: environment,
    stdio: "inherit",
  },
);

if (result.error) {
  console.error(`failed to launch World Explorer smoke: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 1);
