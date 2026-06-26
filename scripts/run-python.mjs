#!/usr/bin/env node
import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";

const args = process.argv.slice(2);
if (args.length === 0) {
  console.error("usage: node scripts/run-python.mjs SCRIPT [ARGS...]");
  process.exit(2);
}

function candidates() {
  const list = [];
  if (process.env.MCLONE_PYTHON) {
    list.push([process.env.MCLONE_PYTHON]);
  }
  if (process.platform === "win32") {
    list.push(["py", "-3"]);
  }
  list.push(["python"]);
  list.push(["python3"]);
  return list;
}

function pythonWorks(command) {
  const probe = spawnSync(
    command[0],
    [
      ...command.slice(1),
      "-c",
      "import sys; sys.exit(0 if sys.version_info[0] == 3 else 1)",
    ],
    {
      stdio: "ignore",
      windowsHide: true,
    },
  );
  return probe.status === 0;
}

function resolvePython() {
  for (const command of candidates()) {
    if (command.length === 1 && command[0].includes("\\") && !existsSync(command[0])) {
      continue;
    }
    if (pythonWorks(command)) {
      return command;
    }
  }

  console.error(
    "Could not find a working Python 3. Install Python, or set MCLONE_PYTHON to a Python 3 executable.",
  );
  process.exit(1);
}

const python = resolvePython();
const result = spawnSync(python[0], [...python.slice(1), ...args], {
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
