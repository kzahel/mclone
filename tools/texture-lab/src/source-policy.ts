import type { TexturePackAsset } from "./dsl";
import type { RenderedTexture } from "./compositor";

export interface SourceNeutralityStats {
  meanSaturation: number;
  maxSaturation: number;
  visibleAlpha: number;
}

export function assertTextureSourcePolicies(pack: TexturePackAsset, textures: RenderedTexture[]): void {
  const errors = textureSourcePolicyErrors(pack, textures);
  if (errors.length > 0) {
    throw new Error(`Texture source policy failed for pack '${pack.name}':\n${errors.map((error) => `- ${error}`).join("\n")}`);
  }
}

export function textureSourcePolicyErrors(pack: TexturePackAsset, textures: RenderedTexture[]): string[] {
  const errors: string[] = [];
  for (const texture of textures) {
    if (texture.source !== "tintable") {
      continue;
    }
    const tintRole = texture.tintRole;
    const neutrality = tintRole ? pack.tints[tintRole]?.sourceNeutrality : undefined;
    if (!tintRole || !neutrality) {
      errors.push(`${texture.name} is tintable but has no sourceNeutrality policy`);
      continue;
    }

    const stats = sourceNeutralityStats(texture);
    if (stats.meanSaturation > neutrality.maxMeanSaturation) {
      errors.push(
        `${texture.name} raw source mean saturation ${formatSaturation(stats.meanSaturation)} exceeds ${formatSaturation(neutrality.maxMeanSaturation)} for tint role '${tintRole}'`,
      );
    }
    if (neutrality.maxPixelSaturation !== undefined && stats.maxSaturation > neutrality.maxPixelSaturation) {
      errors.push(
        `${texture.name} raw source max pixel saturation ${formatSaturation(stats.maxSaturation)} exceeds ${formatSaturation(neutrality.maxPixelSaturation)} for tint role '${tintRole}'`,
      );
    }
  }
  return errors;
}

export function sourceNeutralityStats(texture: RenderedTexture): SourceNeutralityStats {
  let weightedSaturation = 0;
  let visibleAlpha = 0;
  let maxSaturation = 0;
  for (let index = 0; index < texture.data.length; index += 4) {
    const alpha = texture.data[index + 3]! / 255;
    if (alpha <= 0) {
      continue;
    }
    const saturation = pixelSaturation(texture.data[index]!, texture.data[index + 1]!, texture.data[index + 2]!);
    weightedSaturation += saturation * alpha;
    visibleAlpha += alpha;
    maxSaturation = Math.max(maxSaturation, saturation);
  }
  return {
    meanSaturation: visibleAlpha > 0 ? weightedSaturation / visibleAlpha : 0,
    maxSaturation,
    visibleAlpha,
  };
}

function pixelSaturation(red: number, green: number, blue: number): number {
  const max = Math.max(red, green, blue);
  const min = Math.min(red, green, blue);
  return max > 0 ? (max - min) / max : 0;
}

function formatSaturation(value: number): string {
  return value.toFixed(4);
}
