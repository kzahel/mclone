import type { TerrainLabVisualProfile } from "../state";

export const AUTHORED_PACK_URL =
  "/first-party-packs/mclone-authored.pbp";
export const PROVISIONAL_PACK_URL =
  "/first-party-packs/mclone-generated-fallback.pbp";
export const DIAGNOSTIC_PACK_URL =
  "/first-party-packs/mclone-diagnostic-missing.pbp";
export const MINECRAFT_REFERENCE_PACK_URL =
  "/reference/minecraft-1.17.1/extracted.zip";

export interface TerrainVisualAssetBytes {
  authored: Uint8Array;
  reference: Uint8Array | undefined;
  provisional: Uint8Array;
  diagnostic: Uint8Array;
}

let assetsPromise: Promise<TerrainVisualAssetBytes> | undefined;

export function loadTerrainVisualAssets(): Promise<TerrainVisualAssetBytes> {
  assetsPromise ??= Promise.all([
    fetchRequiredPack(AUTHORED_PACK_URL),
    fetchOptionalPack(MINECRAFT_REFERENCE_PACK_URL),
    fetchRequiredPack(PROVISIONAL_PACK_URL),
    fetchRequiredPack(DIAGNOSTIC_PACK_URL),
  ]).then(([authored, reference, provisional, diagnostic]) => ({
    authored,
    reference,
    provisional,
    diagnostic,
  }));
  return assetsPromise;
}

export function visualProfileUsesMinecraftReference(
  profile: TerrainLabVisualProfile,
): boolean {
  return profile === "minecraft-reference" || profile === "hybrid-authoring";
}

export function optionalReferenceBytes(
  assets: TerrainVisualAssetBytes,
): Uint8Array {
  return assets.reference ?? new Uint8Array();
}

async function fetchRequiredPack(url: string): Promise<Uint8Array> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to load ${url}: HTTP ${response.status}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

async function fetchOptionalPack(url: string): Promise<Uint8Array | undefined> {
  try {
    const response = await fetch(url);
    return response.ok
      ? new Uint8Array(await response.arrayBuffer())
      : undefined;
  } catch {
    return undefined;
  }
}
