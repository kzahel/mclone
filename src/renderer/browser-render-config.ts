import { Vec3 } from "../world/phys/vec3";
import type { WorldEngineConfig, WorldEngineLightingMode, WorldEngineLiquidSimulationMode } from "../runtime/protocol/world-messages";

export interface BrowserRenderConfig {
  readonly viewDistance: number;
  readonly renderDistance: number;
  readonly skyColor: Vec3;
  readonly clearColorScale: number;
  readonly lightingMode: WorldEngineLightingMode;
  readonly liquidSimulationMode: WorldEngineLiquidSimulationMode;
}

const DEFAULT_VIEW_DISTANCE = 6;
const DEFAULT_SKY_COLOR = new Vec3(0x8f / 255, 0xb8 / 255, 0xff / 255);
const DEFAULT_CLEAR_COLOR_SCALE = 1.0;
const DEFAULT_LIGHTING_MODE: WorldEngineLightingMode = "vanilla17";
const DEFAULT_LIQUID_SIMULATION_MODE: WorldEngineLiquidSimulationMode = "vanilla17";

export const BROWSER_RENDER_CONFIG_STORAGE_KEY = "mclone.debug.renderConfig.v1";
export const BROWSER_RENDER_CONFIG_QUERY_KEYS = [
  "viewDistance",
  "renderDistance",
  "lightingMode",
  "liquidSimulationMode",
  "disableLighting",
  "disableWaterSim",
] as const;

export interface StoredBrowserRenderConfig extends WorldEngineConfig {
  readonly viewDistance?: number;
  readonly renderDistance?: number;
}

export interface BrowserRenderConfigStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export function getDefaultRenderDistance(viewDistance: number): number {
  return (Math.max(1, viewDistance) + 6) * 16;
}

export function getExpectedLoadedChunkCount(viewDistance: number): number {
  const generatedRadius = Math.max(1, viewDistance) + 1;
  const diameter = (generatedRadius * 2) + 1;
  return diameter * diameter;
}

function clampInteger(value: number, fallback: number, min: number, max: number): number {
  if (!Number.isFinite(value)) {
    return fallback;
  }

  return Math.min(max, Math.max(min, Math.trunc(value)));
}

function parseIntegerValue(raw: string | null, fallback: number, min: number, max: number): number {
  if (raw === null) {
    return fallback;
  }
  const parsed = Number.parseInt(raw, 10);
  return clampInteger(parsed, fallback, min, max);
}

function parseFloatParam(url: URL, key: string, fallback: number, min: number, max: number): number {
  const raw = url.searchParams.get(key);
  if (raw === null) {
    return fallback;
  }

  const parsed = Number.parseFloat(raw);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }

  return Math.min(max, Math.max(min, parsed));
}

function parseHexColor(color: string): Vec3 | undefined {
  const normalized = color.startsWith("#") ? color.slice(1) : color;
  if (!/^[0-9a-fA-F]{6}$/.test(normalized)) {
    return undefined;
  }

  const red = Number.parseInt(normalized.slice(0, 2), 16) / 255;
  const green = Number.parseInt(normalized.slice(2, 4), 16) / 255;
  const blue = Number.parseInt(normalized.slice(4, 6), 16) / 255;
  return new Vec3(red, green, blue);
}

function parseTripletColor(color: string): Vec3 | undefined {
  const parts = color.split(",").map((part) => Number.parseFloat(part.trim()));
  if (parts.length !== 3 || parts.some((part) => !Number.isFinite(part))) {
    return undefined;
  }

  const scale = parts.some((part) => part > 1.0) ? 255 : 1;
  const [red, green, blue] = parts;
  if (red! < 0 || green! < 0 || blue! < 0 || red! > scale || green! > scale || blue! > scale) {
    return undefined;
  }

  return new Vec3(red! / scale, green! / scale, blue! / scale);
}

function parseColorParam(url: URL, key: string, fallback: Vec3): Vec3 {
  const raw = url.searchParams.get(key);
  if (raw === null) {
    return fallback;
  }

  return parseHexColor(raw) ?? parseTripletColor(raw) ?? fallback;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readStoredInteger(record: Record<string, unknown>, key: string, min: number, max: number): number | undefined {
  const value = record[key];
  if (typeof value !== "number") {
    return undefined;
  }

  const clamped = clampInteger(value, Number.NaN, min, max);
  return Number.isNaN(clamped) ? undefined : clamped;
}

function readStoredLightingMode(record: Record<string, unknown>): WorldEngineLightingMode | undefined {
  return record.lightingMode === "none" || record.lightingMode === "vanilla17" ? record.lightingMode : undefined;
}

function readStoredLiquidSimulationMode(record: Record<string, unknown>): WorldEngineLiquidSimulationMode | undefined {
  return record.liquidSimulationMode === "none" || record.liquidSimulationMode === "vanilla17"
    ? record.liquidSimulationMode
    : undefined;
}

export function readStoredBrowserRenderConfig(storage: Pick<BrowserRenderConfigStorage, "getItem"> | undefined): StoredBrowserRenderConfig {
  if (storage === undefined) {
    return {};
  }

  try {
    const raw = storage.getItem(BROWSER_RENDER_CONFIG_STORAGE_KEY);
    if (raw === null) {
      return {};
    }

    const parsed = JSON.parse(raw) as unknown;
    if (!isRecord(parsed)) {
      return {};
    }

    return {
      viewDistance: readStoredInteger(parsed, "viewDistance", 1, 16),
      renderDistance: readStoredInteger(parsed, "renderDistance", 16, 512),
      lightingMode: readStoredLightingMode(parsed),
      liquidSimulationMode: readStoredLiquidSimulationMode(parsed),
    };
  } catch {
    return {};
  }
}

export function writeStoredBrowserRenderConfig(
  storage: Pick<BrowserRenderConfigStorage, "setItem">,
  config: StoredBrowserRenderConfig,
): void {
  storage.setItem(BROWSER_RENDER_CONFIG_STORAGE_KEY, JSON.stringify({
    viewDistance: config.viewDistance,
    renderDistance: config.renderDistance,
    lightingMode: config.lightingMode,
    liquidSimulationMode: config.liquidSimulationMode,
  }));
}

export function clearStoredBrowserRenderConfig(storage: Pick<BrowserRenderConfigStorage, "removeItem">): void {
  storage.removeItem(BROWSER_RENDER_CONFIG_STORAGE_KEY);
}

function parseLightingModeParam(url: URL, fallback: WorldEngineLightingMode): WorldEngineLightingMode {
  const raw = url.searchParams.get("lightingMode");
  if (raw === "none" || raw === "vanilla17") {
    return raw;
  }

  const disable = url.searchParams.get("disableLighting");
  if (disable === "1" || disable === "true") {
    return "none";
  }
  if (disable === "0" || disable === "false") {
    return "vanilla17";
  }

  return fallback;
}

function parseLiquidSimulationModeParam(url: URL, fallback: WorldEngineLiquidSimulationMode): WorldEngineLiquidSimulationMode {
  const raw = url.searchParams.get("liquidSimulationMode");
  if (raw === "none" || raw === "vanilla17") {
    return raw;
  }

  const disable = url.searchParams.get("disableWaterSim");
  if (disable === "1" || disable === "true") {
    return "none";
  }
  if (disable === "0" || disable === "false") {
    return "vanilla17";
  }

  return fallback;
}

export function readBrowserRenderConfig(
  url: URL,
  storage?: Pick<BrowserRenderConfigStorage, "getItem">,
): BrowserRenderConfig {
  const stored = readStoredBrowserRenderConfig(storage);
  const storedViewDistance = stored.viewDistance ?? DEFAULT_VIEW_DISTANCE;
  const viewDistance = parseIntegerValue(url.searchParams.get("viewDistance"), storedViewDistance, 1, 16);
  const storedRenderDistance = stored.renderDistance ?? getDefaultRenderDistance(viewDistance);
  return {
    viewDistance,
    renderDistance: parseIntegerValue(url.searchParams.get("renderDistance"), storedRenderDistance, 16, 512),
    skyColor: parseColorParam(url, "fogColor", DEFAULT_SKY_COLOR),
    clearColorScale: parseFloatParam(url, "clearColorScale", DEFAULT_CLEAR_COLOR_SCALE, 0.0, 1.0),
    lightingMode: parseLightingModeParam(url, stored.lightingMode ?? DEFAULT_LIGHTING_MODE),
    liquidSimulationMode: parseLiquidSimulationModeParam(url, stored.liquidSimulationMode ?? DEFAULT_LIQUID_SIMULATION_MODE),
  };
}
