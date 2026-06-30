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
    lines.push("| Role | Normal | Alternates |");
    lines.push("|---|---|---|");
    for (const [name, tint] of tintEntries) {
      lines.push(`| ${md(name)} | ${md(tint.normal)} | ${md((tint.alternates ?? []).join(", ") || "-")} |`);
    }
  }
  lines.push("");
  lines.push("## Textures");
  lines.push("");
  lines.push("| Texture | Source | Tint Role | Size | Palette | Tiling | Sheet |");
  lines.push("|---|---|---|---|---|---|---|");
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
      lines.push(`Sheet: \`${name}-sheet.png\``);
      if (block.faces.side && block.faces.overlay) {
        lines.push(`Side context sheet: \`${name}-side-sheet.png\``);
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
      return {
        kind: layer.kind,
        rows: layer.pixels.length,
        symbols: Object.keys(layer.colors).sort(),
        opacity: layer.opacity ?? 1,
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
    sheetPath: `${name}-sheet.png`,
    faces: blockFaces(block, pack),
    composition: blockComposition(block, pack),
  };
  if (block.faces.side && block.faces.overlay) {
    report.sideContextSheetPath = `${name}-side-sheet.png`;
  }
  return report;
}

function blockFaces(block: BlockSpec, pack: TexturePackAsset): FaceReport[] {
  return Object.entries(block.faces).map(([role, textureName]) => {
    const texture = pack.textures[textureName];
    return {
      role,
      textureName,
      source: texture ? sourceCategory(texture) : "missing",
      tintRole: texture?.tintRole ?? null,
      tiling: texture?.preview?.tiling ?? "xy",
    };
  });
}

function blockComposition(block: BlockSpec, pack: TexturePackAsset): string[] {
  const lines: string[] = [];
  const top = block.faces.top ?? block.faces.all;
  const bottom = block.faces.bottom ?? block.faces.all;
  const side = block.faces.side ?? block.faces.all;
  const overlay = block.faces.overlay;

  if (top) {
    lines.push(`top = ${textureExpression(top, pack)}`);
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

function sourceCategory(texture: TextureSpec): "final-color" | "tintable" {
  return texture.source ?? "final-color";
}

function md(value: string): string {
  return value.replace(/\|/g, "\\|");
}
