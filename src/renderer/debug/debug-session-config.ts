import type { OpenWorldPreset } from "../../runtime/protocol/world-messages";

export const DEFAULT_DEBUG_SEED = 12_345n;
export const DEFAULT_DEBUG_PRESET: OpenWorldPreset = "browser_smoke";
export const DEFAULT_DEBUG_MOVEMENT_MODE: DebugMovementMode = "player";
export const DEBUG_SESSION_CONFIG_STORAGE_KEY = "mclone.debug.sessionConfig.v1";
export const START_LAST_WORLD_STORAGE_KEY = "mclone.start.lastWorld.v1";
export const DEBUG_SESSION_CONFIG_QUERY_KEYS = ["seed", "movementMode", "preset"] as const;

export type DebugMovementMode = "player" | "freecam";

export interface DebugSessionConfig {
  readonly seed: bigint;
  readonly movementMode: DebugMovementMode;
  readonly preset: OpenWorldPreset;
}

export function parseSeedValue(value: string | null | undefined, fallback: bigint): bigint {
  if (value === null || value === undefined || value.trim() === "") {
    return fallback;
  }

  try {
    return BigInt(value.trim());
  } catch {
    return fallback;
  }
}

export function parseMovementMode(value: unknown, fallback: DebugMovementMode): DebugMovementMode {
  return value === "player" || value === "freecam" ? value : fallback;
}

export function parsePreset(value: unknown, fallback: OpenWorldPreset): OpenWorldPreset {
  return value === "default" || value === "browser_smoke" || value === "flat_grass" || value === "small_island" ? value : fallback;
}

export function readStoredDebugSessionConfig(
  storage: Pick<Storage, "getItem"> | undefined,
): Partial<DebugSessionConfig> {
  if (storage === undefined) {
    return {};
  }

  try {
    const raw = storage.getItem(DEBUG_SESSION_CONFIG_STORAGE_KEY);
    if (raw === null) {
      return {};
    }

    const parsed = JSON.parse(raw) as unknown;
    if (!isRecord(parsed)) {
      return {};
    }

    const movementMode = parsed.movementMode === "player" || parsed.movementMode === "freecam"
      ? parsed.movementMode
      : undefined;
    const preset = parsed.preset === "default" || parsed.preset === "browser_smoke"
      || parsed.preset === "flat_grass" || parsed.preset === "small_island"
      ? parsed.preset
      : undefined;
    return {
      seed: typeof parsed.seed === "string" ? parseSeedValue(parsed.seed, DEFAULT_DEBUG_SEED) : undefined,
      movementMode,
      preset,
    };
  } catch {
    return {};
  }
}

export function readDebugSessionConfig(url: URL, storage?: Pick<Storage, "getItem">): DebugSessionConfig {
  const stored = readStoredDebugSessionConfig(storage);
  const storedSeed = stored.seed ?? DEFAULT_DEBUG_SEED;
  const storedMovementMode = stored.movementMode ?? DEFAULT_DEBUG_MOVEMENT_MODE;
  const storedPreset = stored.preset ?? DEFAULT_DEBUG_PRESET;
  return {
    seed: parseSeedValue(url.searchParams.get("seed"), storedSeed),
    movementMode: parseMovementMode(url.searchParams.get("movementMode"), storedMovementMode),
    preset: parsePreset(url.searchParams.get("preset"), storedPreset),
  };
}

export function writeStoredDebugSessionConfig(
  storage: Pick<Storage, "setItem">,
  config: DebugSessionConfig,
): void {
  storage.setItem(DEBUG_SESSION_CONFIG_STORAGE_KEY, JSON.stringify({
    seed: config.seed.toString(),
    movementMode: config.movementMode,
    preset: config.preset,
  }));
}

export function writeStartLastWorld(
  storage: Pick<Storage, "setItem">,
  config: DebugSessionConfig,
): void {
  storage.setItem(START_LAST_WORLD_STORAGE_KEY, JSON.stringify({
    seed: config.seed.toString(),
    movementMode: config.movementMode,
    preset: config.preset,
    lastOpenedAtMs: Date.now(),
  }));
}

export function clearStoredDebugSessionConfig(storage: Pick<Storage, "removeItem">): void {
  storage.removeItem(DEBUG_SESSION_CONFIG_STORAGE_KEY);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
