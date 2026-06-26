#!/usr/bin/env node
import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";

const args = process.argv.slice(2);
if (args.length === 0) {
  console.error("usage: node scripts/run-native-bash.mjs SCRIPT [ARGS...]");
  process.exit(2);
}

function windowsBashCandidates() {
  return [
    process.env.MCLONE_BASH,
    "C:\\Program Files\\Git\\bin\\bash.exe",
    "C:\\Program Files\\Git\\usr\\bin\\bash.exe",
    "C:\\Program Files (x86)\\Git\\bin\\bash.exe",
  ].filter(Boolean);
}

function resolveBash() {
  if (process.platform !== "win32") {
    return process.env.MCLONE_BASH || "bash";
  }

  for (const candidate of windowsBashCandidates()) {
    if (existsSync(candidate)) {
      return candidate;
    }
  }

  console.error(
    "Git Bash was not found. Install Git for Windows or set MCLONE_BASH to a native bash.exe. Do not use WSL bash for Android cargo builds.",
  );
  process.exit(1);
}

const bash = resolveBash();
const result = spawnSync(bash, args, {
  cwd: process.cwd(),
  env: process.env,
  stdio: "inherit",
  windowsHide: false,
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
