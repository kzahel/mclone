import type { BlockSpec, TexturePackAsset, TextureSpec } from "./dsl";

interface FaceReport {
  role: string;
  textureName: string;
  source: string;
  tintRole: string | null;
  tiling: string;
}

export function makeMetadataReportMarkdown(pack: TexturePackAsset): string {
  const lines: string[] = [];
  lines.push(`# Texture Lab Report: ${pack.name}`);
  lines.push("");
  lines.push(`Default texture size: ${pack.defaultSize}x${pack.defaultSize}`);
  lines.push("");
  lines.push("## Tint Roles");
  lines.push("");
  const tintEntries = Object.entries(pack.tints);
  if (tintEntries.length === 0) {
    lines.push("No tint roles declared.");
  } else {
    lines.push("| Role | Normal | Alternates | Source Neutrality |");
    lines.push("|---|---|---|---|");
    for (const [name, tint] of tintEntries) {
      lines.push(
        `| ${md(name)} | ${md(tint.normal)} | ${md((tint.alternates ?? []).join(", ") || "-")} | ${md(sourceNeutralitySummary(tint))} |`,
      );
    }
  }
  lines.push("");
  lines.push("## Textures");
  lines.push("");
  lines.push("| Texture | Source | Tint Role | Size | Palette | Tiling | Seam Diagnostic | Sheet |");
  lines.push("|---|---|---|---|---|---|---|---|");
  for (const [name, texture] of Object.entries(pack.textures)) {
    const source = sourceCategory(texture);
    lines.push(
      [
        md(name),
        md(source),
        md(texture.tintRole ?? "-"),
        `${textureSize(pack, texture)}x${textureSize(pack, texture)}`,
        md(texture.palette),
        md(texture.preview?.tiling ?? "xy"),
        md(seamDiagnosticMode(texture)),
        md(`${name}-sheet.png`),
      ].join(" | ").replace(/^/, "| ").replace(/$/, " |"),
    );
  }
  lines.push("");
  lines.push("## Blocks");
  lines.push("");
  const blockEntries = Object.entries(pack.blocks);
  if (blockEntries.length === 0) {
    lines.push("No block bundles declared.");
  } else {
    for (const [name, block] of blockEntries) {
      lines.push(`### ${name}`);
      lines.push("");
      lines.push(`Sheet: \`${blockSheetPath(pack, name)}\``);
      if (block.faces.side && block.faces.overlay) {
        lines.push(`Side context sheet: \`${blockSideSheetPath(pack, name)}\``);
      }
      lines.push("");
      lines.push("| Face Role | Texture | Source | Tint Role | Preview Tiling |");
      lines.push("|---|---|---|---|---|");
      for (const face of blockFaces(block, pack)) {
        lines.push(
          `| ${md(face.role)} | ${md(face.textureName)} | ${md(face.source)} | ${md(face.tintRole ?? "-")} | ${md(face.tiling)} |`,
        );
      }
      lines.push("");
      lines.push("Composition:");
      for (const line of blockComposition(block, pack)) {
        lines.push(`- ${line}`);
      }
      lines.push("");
    }
  }

  return `${lines.join("\n").trimEnd()}\n`;
}

export function makeMetadataReportJson(pack: TexturePackAsset): string {
  return `${JSON.stringify(makeMetadataReport(pack), null, 2)}\n`;
}

function makeMetadataReport(pack: TexturePackAsset): Record<string, unknown> {
  return {
    schemaVersion: 1,
    pack: {
      name: pack.name,
      defaultSize: pack.defaultSize,
    },
    tints: Object.fromEntries(
      Object.entries(pack.tints).map(([name, tint]) => [
        name,
        {
          normal: tint.normal,
          alternates: tint.alternates ?? [],
          sourceNeutrality: tint.sourceNeutrality ?? null,
        },
      ]),
    ),
    textures: Object.entries(pack.textures).map(([name, texture]) => textureReport(pack, name, texture)),
    blocks: Object.entries(pack.blocks).map(([name, block]) => blockReport(pack, name, block)),
  };
}

function textureReport(pack: TexturePackAsset, name: string, texture: TextureSpec): Record<string, unknown> {
  const report: Record<string, unknown> = {
    name,
    size: textureSize(pack, texture),
    source: sourceCategory(texture),
    palette: texture.palette,
    base: texture.base,
    exportPath: texture.exportPath,
    sheetPath: `${name}-sheet.png`,
    preview: {
      tiling: texture.preview?.tiling ?? "xy",
      seamDiagnostic: seamDiagnosticMode(texture),
      checkerboard: texture.preview?.checkerboard ?? false,
      cube: texture.preview?.cube ?? "auto",
      rotation: texture.preview?.rotation ?? "auto",
    },
    layers: (texture.layers ?? []).map((layer) => {
      if (layer.kind === "speckles") {
        return {
          kind: layer.kind,
          seed: layer.seed,
          density: layer.density,
          colors: layer.colors,
          opacity: layer.opacity ?? 1,
          radius: layer.radius ?? 0,
        };
      }
      if (layer.kind === "macroNoise") {
        return {
          kind: layer.kind,
          seed: layer.seed,
          frequency: layer.frequency,
          octaves: layer.octaves ?? 1,
          colors: layer.colors,
          opacity: layer.opacity ?? 1,
          contrast: layer.contrast ?? 1,
          bias: layer.bias ?? 0,
        };
      }
      return {
        kind: layer.kind,
        rows: layer.pixels.length,
        symbols: Object.keys(layer.colors).sort(),
        opacity: layer.opacity ?? 1,
        upscale: layer.kind === "mask" ? layer.upscale ?? "nearest" : undefined,
      };
    }),
  };
  if (texture.tintRole) {
    report.tintRole = texture.tintRole;
    report.tint = pack.tints[texture.tintRole] ?? null;
  }
  return report;
}

function blockReport(pack: TexturePackAsset, name: string, block: BlockSpec): Record<string, unknown> {
  const report: Record<string, unknown> = {
    name,
    kind: block.kind,
    sheetPath: blockSheetPath(pack, name),
    faces: blockFaces(block, pack),
    composition: blockComposition(block, pack),
  };
  if (block.faces.side && block.faces.overlay) {
    report.sideContextSheetPath = blockSideSheetPath(pack, name);
  }
  return report;
}

function blockSheetPath(pack: TexturePackAsset, blockName: string): string {
  return pack.textures[blockName] ? `${blockName}-block-sheet.png` : `${blockName}-sheet.png`;
}

function blockSideSheetPath(pack: TexturePackAsset, blockName: string): string {
  return pack.textures[blockName] ? `${blockName}-block-side-sheet.png` : `${blockName}-side-sheet.png`;
}

function blockFaces(block: BlockSpec, pack: TexturePackAsset): FaceReport[] {
  return Object.entries(block.faces)
    .map(([role, textureName]) => {
      const texture = pack.textures[textureName];
      return {
        role,
        textureName,
        source: texture ? sourceCategory(texture) : "missing",
        tintRole: texture?.tintRole ?? null,
        tiling: texture?.preview?.tiling ?? "xy",
      };
    })
    .sort((left, right) => blockFaceOrder(left.role) - blockFaceOrder(right.role) || left.role.localeCompare(right.role));
}

function blockComposition(block: BlockSpec, pack: TexturePackAsset): string[] {
  if (block.kind === "cross") {
    const textureName = block.faces.all ?? block.faces.side ?? block.faces.top;
    return textureName ? [`cross planes = ${textureExpression(textureName, pack)}`] : ["cross block has no billboard texture"];
  }
  if (block.kind === "flat") {
    const textureName = block.faces.top ?? block.faces.all;
    return textureName ? [`ground plane = ${textureExpression(textureName, pack)}`] : ["flat block has no top texture"];
  }
  if (block.kind === "pane") {
    const pane = block.faces.side ?? block.faces.all;
    const edge = block.faces.top ?? block.faces.bottom;
    return [
      pane ? `thin pane faces = ${textureExpression(pane, pack)}` : "pane block has no pane texture",
      edge ? `edge/post = ${textureExpression(edge, pack)}` : "edge/post uses pane texture",
    ];
  }
  if (block.kind === "rail") {
    const textureName = block.faces.top ?? block.faces.all;
    return textureName ? [`flat and raised rail planes = ${textureExpression(textureName, pack)}`] : ["rail block has no top texture"];
  }
  if (block.kind === "torch") {
    const textureName = block.faces.side ?? block.faces.all ?? block.faces.top;
    return textureName ? [`standing and wall torch billboards = ${textureExpression(textureName, pack)}`] : ["torch block has no sprite texture"];
  }
  if (block.kind === "door") {
    const lines: string[] = [];
    if (block.faces.top) {
      lines.push(`upper half = ${textureExpression(block.faces.top, pack)}`);
    }
    if (block.faces.bottom) {
      lines.push(`lower half = ${textureExpression(block.faces.bottom, pack)}`);
    }
    return lines.length ? lines : ["door block has no upper/lower textures"];
  }
  if (block.kind === "trapdoor") {
    const textureName = block.faces.top ?? block.faces.all;
    return textureName ? [`closed slab and open panel = ${textureExpression(textureName, pack)}`] : ["trapdoor block has no texture"];
  }

  const lines: string[] = [];
  const top = block.faces.top ?? block.faces.all;
  const bottom = block.faces.bottom ?? block.faces.all;
  const side = block.faces.side ?? block.faces.all;
  const overlay = block.faces.overlay;
  const directionalFaces = ["north", "east", "south", "west"] as const;

  if (top) {
    lines.push(`top = ${textureExpression(top, pack)}`);
  }
  for (const face of directionalFaces) {
    const textureName = block.faces[face];
    if (textureName) {
      lines.push(`${face} = ${textureExpression(textureName, pack)}`);
    }
  }
  if (side && overlay) {
    lines.push(`side = ${textureExpression(side, pack)} + ${textureExpression(overlay, pack)}`);
  } else if (side) {
    lines.push(`side = ${textureExpression(side, pack)}`);
  }
  if (bottom) {
    lines.push(`bottom = ${textureExpression(bottom, pack)}`);
  }

  return lines.length > 0 ? lines : ["no face composition available"];
}

function blockFaceOrder(face: string): number {
  const order = ["all", "top", "bottom", "north", "east", "south", "west", "side", "overlay", "particle"];
  const index = order.indexOf(face);
  return index === -1 ? order.length : index;
}

function textureExpression(textureName: string, pack: TexturePackAsset): string {
  const texture = pack.textures[textureName];
  if (!texture) {
    return `${textureName} (missing texture)`;
  }
  if (sourceCategory(texture) !== "tintable") {
    return `${textureName} as-authored`;
  }
  const tintRole = texture.tintRole ?? "missing tint role";
  const tint = texture.tintRole ? pack.tints[texture.tintRole] : undefined;
  const normal = tint ? ` ${tint.normal}` : "";
  return `${textureName} multiplied by ${tintRole}${normal}`;
}

function textureSize(pack: TexturePackAsset, texture: TextureSpec): number {
  return texture.size ?? pack.defaultSize;
}

function seamDiagnosticMode(texture: TextureSpec): string {
  const tiling = texture.preview?.tiling ?? "xy";
  if (tiling === "none") {
    return "disabled";
  }
  if (tiling === "x") {
    return "left-right";
  }
  return "left-right/top-bottom";
}

function sourceCategory(texture: TextureSpec): "final-color" | "tintable" {
  return texture.source ?? "final-color";
}

function sourceNeutralitySummary(tint: TexturePackAsset["tints"][string]): string {
  if (!tint.sourceNeutrality) {
    return "-";
  }
  const parts = [`mean <= ${tint.sourceNeutrality.maxMeanSaturation}`];
  if (tint.sourceNeutrality.maxPixelSaturation !== undefined) {
    parts.push(`pixel <= ${tint.sourceNeutrality.maxPixelSaturation}`);
  }
  return parts.join(", ");
}

function md(value: string): string {
  return value.replace(/\|/g, "\\|");
}
