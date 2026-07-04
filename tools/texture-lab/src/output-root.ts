import path from "node:path";
import { fileURLToPath } from "node:url";

export const TEXTURE_LAB_OUTPUT_ROOT_ENV = "MCLONE_TEXTURE_LAB_OUTPUT_ROOT";

export function textureLabOutputRoot(env: NodeJS.ProcessEnv = process.env): string {
  const override = env[TEXTURE_LAB_OUTPUT_ROOT_ENV]?.trim();
  if (override) {
    return path.resolve(override);
  }
  return defaultTextureLabOutputRoot();
}

export function textureLabOutputPath(...parts: string[]): string {
  return path.join(textureLabOutputRoot(), ...parts);
}

export function defaultTextureLabOutputRoot(): string {
  return path.join(repoRoot(), "generated-assets", "texture-lab");
}

export function repoRoot(): string {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
}
