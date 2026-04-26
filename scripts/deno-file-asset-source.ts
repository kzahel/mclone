import { FileAssetPack, type FileAssetPackHost } from "../src/renderer/assets/file-asset-pack.ts";

export const DEFAULT_DENO_EXTRACTED_ASSETS_ROOT = new URL("../reference/minecraft-1.17.1/extracted/", import.meta.url);

const DENO_FILE_ASSET_PACK_HOST: FileAssetPackHost = {
  isFile(path): boolean {
    return Deno.statSync(path).isFile;
  },

  readText(path): Promise<string> {
    return Deno.readTextFile(path);
  },

  readBytes(path): Promise<Uint8Array> {
    return Deno.readFile(path);
  },

  isNotFound(error): boolean {
    return error instanceof Deno.errors.NotFound;
  },
};

export function createDenoExtractedAssetPack(root: URL = DEFAULT_DENO_EXTRACTED_ASSETS_ROOT): FileAssetPack {
  return new FileAssetPack(root, DENO_FILE_ASSET_PACK_HOST);
}
