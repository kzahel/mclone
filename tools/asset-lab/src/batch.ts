import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import type { FigureAsset } from "./dsl";
import { loadFigureAsset } from "./load";

interface BatchArgs {
  cycles: number;
  inputs: string[];
  outRoot: string;
  sheets: boolean;
  videos: boolean;
}

const execFileAsync = promisify(execFile);
const assetLabRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const args = parseArgs(process.argv.slice(2));
const inputs = args.inputs.length > 0 ? args.inputs : await discoverExamples();

if (inputs.length === 0) {
  throw new Error("No figure examples found");
}

await fs.mkdir(args.outRoot, { recursive: true });

for (const input of inputs) {
  const asset = await loadFigureAsset(input);
  const clipName = chooseClip(asset);
  const figureOutDir = path.join(args.outRoot, asset.name);
  console.log(`[${asset.name}] export -> ${figureOutDir}`);
  await runTsx("src/export.ts", input, "--out", figureOutDir);

  if (args.sheets) {
    const sheetPath = path.join(args.outRoot, `${asset.name}-sheet.png`);
    console.log(`[${asset.name}] sheet ${clipName} -> ${sheetPath}`);
    await runTsx("src/sheet.ts", input, "--clip", clipName, "--out", sheetPath);
  }

  if (args.videos) {
    const videoPath = path.join(args.outRoot, `${asset.name}-${clipName}.mp4`);
    console.log(`[${asset.name}] video ${clipName} -> ${videoPath}`);
    await runTsx(
      "src/video.ts",
      input,
      "--clip",
      clipName,
      "--cycles",
      String(args.cycles),
      "--out",
      videoPath,
    );
  }
}

function parseArgs(argv: string[]): BatchArgs {
  const inputs: string[] = [];
  let cycles = 4;
  let outRoot = path.join("/tmp", "mclone-asset-lab");
  let sheets = true;
  let videos = true;

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg) {
      continue;
    }
    if (arg === "--cycles") {
      cycles = parsePositiveNumber(requiredArg(argv[index + 1], "--cycles"), "--cycles");
      index += 1;
    } else if (arg === "--out-root") {
      outRoot = requiredArg(argv[index + 1], "--out-root");
      index += 1;
    } else if (arg === "--no-sheet") {
      sheets = false;
    } else if (arg === "--no-video") {
      videos = false;
    } else if (arg.startsWith("-")) {
      throw new Error(`Unknown argument '${arg}'`);
    } else {
      inputs.push(arg);
    }
  }

  return { cycles, inputs, outRoot, sheets, videos };
}

function requiredArg(value: string | undefined, flag: string): string {
  if (!value) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
}

async function discoverExamples(): Promise<string[]> {
  const examplesDir = path.join(assetLabRoot, "examples");
  const entries = await fs.readdir(examplesDir, { withFileTypes: true });
  const figures: string[] = [];
  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }
    const figurePath = path.join("examples", entry.name, "figure.ts");
    try {
      await fs.access(path.join(assetLabRoot, figurePath));
      figures.push(figurePath);
    } catch {
      // Ignore folders that are not figure examples.
    }
  }
  return figures.sort();
}

function chooseClip(asset: FigureAsset): string {
  const clipNames = Object.keys(asset.clips);
  if (clipNames.includes("walk")) {
    return "walk";
  }
  if (clipNames.includes("fly")) {
    return "fly";
  }
  const first = clipNames[0];
  if (!first) {
    throw new Error(`Figure '${asset.name}' has no clips`);
  }
  return first;
}

async function runTsx(script: string, ...scriptArgs: string[]): Promise<void> {
  const { stdout, stderr } = await execFileAsync(tsxBin(), [script, ...scriptArgs], {
    cwd: assetLabRoot,
    maxBuffer: 4 * 1024 * 1024,
    timeout: 240_000,
  });
  if (stdout.trim()) {
    console.log(stdout.trim());
  }
  if (stderr.trim()) {
    console.error(stderr.trim());
  }
}

function tsxBin(): string {
  return process.platform === "win32" ? "tsx.cmd" : "tsx";
}

function parsePositiveNumber(value: string | undefined, flag: string): number {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${flag} must be a positive number`);
  }
  return parsed;
}
