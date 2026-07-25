import path from "node:path";
import { pathToFileURL } from "node:url";
import { applyFrozenTextureCurationToPack } from "./core/frozen-curation";
import { applyTextureLifecycleToPack } from "./core/texture-lifecycle";
import { assertValidTexturePack, type TexturePackAsset } from "./dsl";

export async function loadTexturePack(inputPath: string): Promise<TexturePackAsset> {
  const absolutePath = path.resolve(inputPath);
  const moduleUrl = pathToFileURL(absolutePath).href;
  const module = (await import(moduleUrl)) as { default?: unknown; asset?: unknown };
  const asset = module.default ?? module.asset;

  if (!isTexturePack(asset)) {
    throw new Error(`Expected '${inputPath}' to export a TexturePackAsset as default`);
  }

  assertValidTexturePack(asset);
  const frozen = await applyFrozenTextureCurationToPack(absolutePath, asset);
  return applyTextureLifecycleToPack(absolutePath, frozen);
}

function isTexturePack(value: unknown): value is TexturePackAsset {
  if (!value || typeof value !== "object") {
    return false;
  }
  const asset = value as Partial<TexturePackAsset>;
  return asset.schemaVersion === 1 && typeof asset.name === "string" && typeof asset.textures === "object";
}
