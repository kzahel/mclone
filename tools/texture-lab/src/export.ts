import fs from "node:fs/promises";
import path from "node:path";
import { loadTexturePack } from "./load";
import { makeLodMaterialsJson } from "./lod-materials";
import { encodePng } from "./png";
import { loadReferenceTexture, runtimeCompatTexturePath } from "./reference";
import { makeMetadataReportJson, makeMetadataReportMarkdown } from "./report";
import { makeBlockReviewSheet, makeBlockSideReviewSheet, makeReviewSheet, renderAllTextures } from "./render";

interface ExportArgs {
  input: string;
  outDir: string;
  sheetOnly: boolean;
  runtimeCompat: boolean;
  includeReference: boolean;
  referenceRoot: string | undefined;
}

const args = parseArgs(process.argv.slice(2));
const pack = await loadTexturePack(args.input);
const textures = renderAllTextures(pack);
const texturesByName = new Map(textures.map((texture) => [texture.name, texture]));

await fs.mkdir(args.outDir, { recursive: true });

for (const texture of textures) {
  if (!args.sheetOnly) {
    const outputPath = path.join(args.outDir, "pack", texture.exportPath);
    await fs.mkdir(path.dirname(outputPath), { recursive: true });
    await fs.writeFile(outputPath, encodePng(texture));
    console.log(`Wrote ${outputPath}`);

    if (args.runtimeCompat) {
      const compatPath = runtimeCompatTexturePath(texture.exportPath);
      if (compatPath) {
        const compatOutputPath = path.join(args.outDir, "runtime-pack", compatPath);
        await fs.mkdir(path.dirname(compatOutputPath), { recursive: true });
        await fs.writeFile(compatOutputPath, encodePng(texture));
        console.log(`Wrote ${compatOutputPath}`);
      }
    }
  }

  const sheetPath = path.join(args.outDir, `${texture.name}-sheet.png`);
  const reference = await loadReferenceTexture(texture.exportPath, {
    include: args.includeReference,
    referenceRoot: args.referenceRoot,
  });
  await fs.writeFile(sheetPath, encodePng(makeReviewSheet(texture, reference ? { reference } : {})));
  console.log(`Wrote ${sheetPath}`);
}

for (const [blockName, block] of Object.entries(pack.blocks)) {
  const blockSheetName = texturesByName.has(blockName) ? `${blockName}-block-sheet.png` : `${blockName}-sheet.png`;
  const sheetPath = path.join(args.outDir, blockSheetName);
  await fs.writeFile(sheetPath, encodePng(makeBlockReviewSheet(blockName, block, texturesByName)));
  console.log(`Wrote ${sheetPath}`);

  const sideSheet = makeBlockSideReviewSheet(blockName, block, texturesByName);
  if (sideSheet) {
    const sideSheetName = texturesByName.has(blockName) ? `${blockName}-block-side-sheet.png` : `${blockName}-side-sheet.png`;
    const sideSheetPath = path.join(args.outDir, sideSheetName);
    await fs.writeFile(sideSheetPath, encodePng(sideSheet));
    console.log(`Wrote ${sideSheetPath}`);
  }
}

const reportMarkdownPath = path.join(args.outDir, `${pack.name}-metadata.md`);
await fs.writeFile(reportMarkdownPath, makeMetadataReportMarkdown(pack));
console.log(`Wrote ${reportMarkdownPath}`);

const reportJsonPath = path.join(args.outDir, `${pack.name}-metadata.json`);
await fs.writeFile(reportJsonPath, makeMetadataReportJson(pack));
console.log(`Wrote ${reportJsonPath}`);

const lodMaterialsJson = makeLodMaterialsJson(pack, textures);
const lodMaterialsReportPath = path.join(args.outDir, `${pack.name}-lod-materials.v1.json`);
await fs.writeFile(lodMaterialsReportPath, lodMaterialsJson);
console.log(`Wrote ${lodMaterialsReportPath}`);

if (!args.sheetOnly && args.runtimeCompat) {
  const lodMaterialsPackPath = path.join(args.outDir, "runtime-pack", "assets/mclone/lod/materials.v1.json");
  await fs.mkdir(path.dirname(lodMaterialsPackPath), { recursive: true });
  await fs.writeFile(lodMaterialsPackPath, lodMaterialsJson);
  console.log(`Wrote ${lodMaterialsPackPath}`);
}

function parseArgs(argv: string[]): ExportArgs {
  const input = argv[0];
  if (!input || input.startsWith("-")) {
    throw new Error(
      "Usage: tsx src/export.ts <texture.ts> [--out <dir>] [--sheet-only] [--runtime-compat] [--reference-root <dir>] [--no-reference]",
    );
  }

  let outDir = path.join("/tmp", "mclone-texture-lab");
  let sheetOnly = false;
  let runtimeCompat = false;
  let includeReference = true;
  let referenceRoot: string | undefined;
  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--") {
      continue;
    } else if (arg === "--out") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--out requires a directory");
      }
      outDir = next;
      index += 1;
    } else if (arg === "--reference-root") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--reference-root requires a directory");
      }
      referenceRoot = next;
      index += 1;
    } else if (arg === "--sheet-only") {
      sheetOnly = true;
    } else if (arg === "--runtime-compat") {
      runtimeCompat = true;
    } else if (arg === "--no-reference") {
      includeReference = false;
    } else {
      throw new Error(`Unknown argument '${arg}'`);
    }
  }

  return { input, outDir, sheetOnly, runtimeCompat, includeReference, referenceRoot };
}
