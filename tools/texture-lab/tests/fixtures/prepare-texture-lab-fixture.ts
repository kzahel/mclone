import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const textureLabRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const repoRoot = path.resolve(textureLabRoot, "..", "..");
const outputRoot = path.resolve(
  process.env.MCLONE_TEXTURE_LAB_OUTPUT_ROOT ?? path.join(repoRoot, "generated-assets", "texture-lab-playwright"),
);

await fs.rm(outputRoot, { recursive: true, force: true });
await fs.mkdir(outputRoot, { recursive: true });

await run("pnpm", [
  "exec",
  "tsx",
  "src/export.ts",
  "packs/mclone-default/texture.ts",
  "--out",
  outputRoot,
  "--runtime-compat",
  "--no-reference",
  "--texture",
  "grass_block_top",
  "--texture",
  "stone",
  "--texture",
  "pumpkin_top",
  "--texture",
  "pumpkin_side",
  "--texture",
  "carved_pumpkin",
  "--texture",
  "grass_cross",
  "--texture",
  "fern_cross",
  "--texture",
  "redstone_dust_dot",
  "--texture",
  "glass",
  "--texture",
  "glass_pane_top",
  "--texture",
  "rail",
  "--texture",
  "torch",
  "--texture",
  "oak_door_top",
  "--texture",
  "oak_door_bottom",
  "--texture",
  "oak_trapdoor",
]);

await writeCandidateArtifacts();

async function writeCandidateArtifacts(): Promise<void> {
  const grassPng = path.join(outputRoot, "pack", "assets/mclone/textures/block/grass_block_top.png");
  const grassSheet = path.join(outputRoot, "grass_block_top-sheet.png");
  const diffusionDir = path.join(outputRoot, "diffusion", "grass-playwright");
  const projectionDir = path.join(outputRoot, "diffusion-projection", "grass-playwright");
  const archiveDir = path.join(outputRoot, "diffusion-archive", "grass-playwright");
  const primaryId = "candidate-seed5101-strength0p740";
  const secondaryId = "candidate-seed5102-strength0p620";

  await fs.mkdir(diffusionDir, { recursive: true });
  await copyMany(grassPng, [
    path.join(diffusionDir, "input-prepared.png"),
    path.join(diffusionDir, `${primaryId}.png`),
    path.join(diffusionDir, `${primaryId}-tile3x3.png`),
    path.join(diffusionDir, `${secondaryId}.png`),
    path.join(diffusionDir, `${secondaryId}-tile3x3.png`),
    path.join(diffusionDir, "contact-sheet.png"),
  ]);

  const diffusionManifestPath = path.join(diffusionDir, "manifest.json");
  await writeJson(diffusionManifestPath, {
    input: {
      path: grassPng,
      prepared_png: path.join(diffusionDir, "input-prepared.png"),
    },
    model_id: "playwright-fixture-model",
    scheduler: "PNDMScheduler",
    steps: 24,
    prompt_preset: "grass-top-tufts",
    prompt: "photographic top-down macro grass, primitive rough texture, tileable meadow surface",
    negative_prompt: "text, watermark, frame",
    contact_sheet_png: path.join(diffusionDir, "contact-sheet.png"),
    candidates: [
      {
        id: primaryId,
        png: path.join(diffusionDir, `${primaryId}.png`),
        tile3x3_png: path.join(diffusionDir, `${primaryId}-tile3x3.png`),
        seed: 5101,
        strength: 0.74,
      },
      {
        id: secondaryId,
        png: path.join(diffusionDir, `${secondaryId}.png`),
        tile3x3_png: path.join(diffusionDir, `${secondaryId}-tile3x3.png`),
        seed: 5102,
        strength: 0.62,
      },
    ],
  });

  const projectedCandidateDir = path.join(projectionDir, primaryId);
  await fs.mkdir(projectedCandidateDir, { recursive: true });
  await copyMany(grassPng, [
    path.join(projectedCandidateDir, `${primaryId}-64.png`),
    path.join(projectionDir, "projection-review-64.png"),
  ]);
  await fs.writeFile(path.join(projectedCandidateDir, `${primaryId}-64.mask.txt`), "playwright-mask\n");
  await writeJson(path.join(projectedCandidateDir, `${primaryId}-projection-report.json`), {
    id: primaryId,
    codename: "G5101S74",
    resolution: 64,
    status: "keep",
    score: 0.42,
  });
  await writeJson(path.join(projectionDir, "projection-summary.json"), {
    sourceManifest: diffusionManifestPath,
    texture: "grass_block_top",
    paletteColors: ["#426d33", "#6f9b50", "#a9c77d", "#d7e6ad"],
    candidates: [
      {
        id: primaryId,
        codename: "G5101S74",
        reports: [
          {
            resolution: 64,
            status: "keep",
            score: 0.42,
            reasons: ["playwright projection accepted"],
            png: path.join(projectedCandidateDir, `${primaryId}-64.png`),
            mask: path.join(projectedCandidateDir, `${primaryId}-64.mask.txt`),
          },
        ],
      },
    ],
  });

  const archivedCandidateDir = path.join(archiveDir, "candidates", primaryId);
  await fs.mkdir(archivedCandidateDir, { recursive: true });
  await copyMany(grassPng, [
    path.join(archivedCandidateDir, "raw.png"),
    path.join(archivedCandidateDir, "raw-tile3x3.png"),
    path.join(archivedCandidateDir, "projected-64.png"),
  ]);
  await fs.writeFile(path.join(archivedCandidateDir, "projected-64.mask.txt"), "playwright-archive-mask\n");
  await writeJson(path.join(archivedCandidateDir, "projection-report.json"), {
    id: primaryId,
    codename: "G5101S74",
    resolution: 64,
    status: "keep",
    score: 0.27,
  });
  await copyMany(grassSheet, [path.join(archiveDir, "projection-review-64.png")]);
  await writeJson(path.join(archiveDir, "archive-manifest.json"), {
    texture: "grass_block_top",
    palette: {
      colors: ["#426d33", "#6f9b50", "#a9c77d", "#d7e6ad"],
    },
    diffusion: {
      manifest: { path: diffusionManifestPath },
      modelId: "playwright-fixture-model",
      promptPreset: "grass-top-tufts",
      prompt: "photographic top-down macro grass, primitive rough texture, tileable meadow surface",
      negativePrompt: "text, watermark, frame",
      scheduler: "PNDMScheduler",
      steps: 24,
    },
    reviewSheet: { path: path.join(archiveDir, "projection-review-64.png") },
    candidates: [
      {
        id: primaryId,
        codename: "G5101S74",
        seed: 5101,
        strength: 0.74,
        raw: { path: path.join(archivedCandidateDir, "raw.png") },
        rawTile: { path: path.join(archivedCandidateDir, "raw-tile3x3.png") },
        projectionReport: { path: path.join(archivedCandidateDir, "projection-report.json") },
        resolutions: [
          {
            resolution: 64,
            png: { path: path.join(archivedCandidateDir, "projected-64.png") },
            triage: {
              status: "keep",
              score: 0.27,
              reasons: ["playwright archive accepted"],
            },
          },
        ],
      },
    ],
  });
}

async function copyMany(source: string, destinations: string[]): Promise<void> {
  for (const destination of destinations) {
    await fs.mkdir(path.dirname(destination), { recursive: true });
    await fs.copyFile(source, destination);
  }
}

async function writeJson(filePath: string, value: unknown): Promise<void> {
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function run(command: string, args: string[]): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: textureLabRoot,
      env: process.env,
      stdio: "inherit",
    });
    child.once("error", reject);
    child.once("exit", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`${command} ${args.join(" ")} exited with ${code}`));
      }
    });
  });
}
