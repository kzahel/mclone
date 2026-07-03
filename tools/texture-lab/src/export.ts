import fs from "node:fs/promises";
import path from "node:path";
import type { AuthoringLayerRole } from "./dsl";
import { loadTexturePack } from "./load";
import { makeLodMaterialsJson } from "./lod-materials";
import { encodePng } from "./png";
import { loadReferenceTexture, runtimeCompatTexturePath } from "./reference";
import { makeMetadataReportJson, makeMetadataReportMarkdown } from "./report";
import { makeBlockReviewSheet, makeBlockSideReviewSheet, makeReviewSheet, renderAllTextures, type RenderedTexture } from "./render";

interface ExportArgs {
  input: string;
  outDir: string;
  textures: string[];
  sheetOnly: boolean;
  authoringOnly: boolean;
  authoringRoles: AuthoringLayerRole[];
  runtimeCompat: boolean;
  includeReference: boolean;
  referenceRoot: string | undefined;
}

const args = parseArgs(process.argv.slice(2));
const pack = await loadTexturePack(args.input);
const allTextures = renderAllTextures(pack);
const textures = allTextures.filter((texture) => args.textures.length === 0 || args.textures.includes(texture.name));
const texturesByName = new Map(allTextures.map((texture) => [texture.name, texture]));

if (textures.length === 0) {
  throw new Error(`No textures matched ${JSON.stringify(args.textures)} in pack '${pack.name}'`);
}

await fs.mkdir(args.outDir, { recursive: true });

for (const texture of textures) {
  if (args.authoringRoles.length > 0) {
    await writeAuthoringPreviews(texture, args);
  }

  if (!args.authoringOnly && !args.sheetOnly) {
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

  if (!args.authoringOnly) {
    const sheetPath = path.join(args.outDir, `${texture.name}-sheet.png`);
    const reference = await loadReferenceTexture(texture.exportPath, {
      include: args.includeReference,
      referenceRoot: args.referenceRoot,
    });
    await fs.writeFile(sheetPath, encodePng(makeReviewSheet(texture, reference ? { reference } : {})));
    console.log(`Wrote ${sheetPath}`);
  }
}

if (!args.authoringOnly) {
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

  const lodMaterialsJson = makeLodMaterialsJson(pack, allTextures);
  const lodMaterialsReportPath = path.join(args.outDir, `${pack.name}-lod-materials.v1.json`);
  await fs.writeFile(lodMaterialsReportPath, lodMaterialsJson);
  console.log(`Wrote ${lodMaterialsReportPath}`);

  if (!args.sheetOnly && args.runtimeCompat) {
    const lodMaterialsPackPath = path.join(args.outDir, "runtime-pack", "assets/mclone/lod/materials.v1.json");
    await fs.mkdir(path.dirname(lodMaterialsPackPath), { recursive: true });
    await fs.writeFile(lodMaterialsPackPath, lodMaterialsJson);
    console.log(`Wrote ${lodMaterialsPackPath}`);
  }
}

function parseArgs(argv: string[]): ExportArgs {
  const input = argv[0];
  if (!input || input.startsWith("-")) {
    throw new Error(
      "Usage: tsx src/export.ts <texture.ts> [--out <dir>] [--texture <name>]... [--sheet-only] [--authoring-role <role>]... [--authoring-only] [--runtime-compat] [--reference-root <dir>] [--no-reference]",
    );
  }

  let outDir = path.join("/tmp", "mclone-texture-lab");
  const textures: string[] = [];
  let sheetOnly = false;
  let authoringOnly = false;
  const authoringRoles: AuthoringLayerRole[] = [];
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
    } else if (arg === "--texture") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--texture requires a name");
      }
      textures.push(next);
      index += 1;
    } else if (arg === "--authoring-role") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--authoring-role requires a role");
      }
      if (!isAuthoringLayerRole(next)) {
        throw new Error(`Unsupported authoring role '${next}'`);
      }
      authoringRoles.push(next);
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
    } else if (arg === "--authoring-only") {
      authoringOnly = true;
    } else if (arg === "--runtime-compat") {
      runtimeCompat = true;
    } else if (arg === "--no-reference") {
      includeReference = false;
    } else {
      throw new Error(`Unknown argument '${arg}'`);
    }
  }

  if (authoringOnly && authoringRoles.length === 0) {
    throw new Error("--authoring-only requires at least one --authoring-role");
  }

  return {
    input,
    outDir,
    textures,
    sheetOnly,
    authoringOnly,
    authoringRoles,
    runtimeCompat,
    includeReference,
    referenceRoot,
  };
}

async function writeAuthoringPreviews(texture: RenderedTexture, args: ExportArgs): Promise<void> {
  const matches = texture.authoring.filter((preview) => args.authoringRoles.includes(preview.role));
  if (matches.length === 0) {
    throw new Error(`Texture '${texture.name}' has no authoring preview for ${args.authoringRoles.join(", ")}`);
  }
  for (const [index, preview] of matches.entries()) {
    const suffix = matches.length === 1 ? preview.role : `${preview.role}-${index}`;
    const outputPath = path.join(args.outDir, "authoring", `${texture.name}-${suffix}.png`);
    await fs.mkdir(path.dirname(outputPath), { recursive: true });
    await fs.writeFile(outputPath, encodePng(preview));
    console.log(`Wrote ${outputPath}`);
  }
}

function isAuthoringLayerRole(value: string): value is AuthoringLayerRole {
  return value === "structure";
}
