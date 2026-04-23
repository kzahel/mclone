import { Vec3 } from "../world/phys/vec3";

export interface BrowserRenderConfig {
  readonly viewDistance: number;
  readonly renderDistance: number;
  readonly skyColor: Vec3;
  readonly clearColorScale: number;
}

const DEFAULT_VIEW_DISTANCE = 6;
const DEFAULT_SKY_COLOR = new Vec3(0x8f / 255, 0xb8 / 255, 0xff / 255);
const DEFAULT_CLEAR_COLOR_SCALE = 1.0;

export function getDefaultRenderDistance(viewDistance: number): number {
  return (Math.max(1, viewDistance) + 6) * 16;
}

export function getExpectedLoadedChunkCount(viewDistance: number): number {
  const generatedRadius = Math.max(1, viewDistance) + 1;
  const diameter = (generatedRadius * 2) + 1;
  return diameter * diameter;
}

function parseIntegerParam(url: URL, key: string, fallback: number, min: number, max: number): number {
  const raw = url.searchParams.get(key);
  if (raw === null) {
    return fallback;
  }

  const parsed = Number.parseInt(raw, 10);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }

  return Math.min(max, Math.max(min, parsed));
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

export function readBrowserRenderConfig(url: URL): BrowserRenderConfig {
  const viewDistance = parseIntegerParam(url, "viewDistance", DEFAULT_VIEW_DISTANCE, 1, 16);
  return {
    viewDistance,
    renderDistance: parseIntegerParam(url, "renderDistance", getDefaultRenderDistance(viewDistance), 16, 512),
    skyColor: parseColorParam(url, "fogColor", DEFAULT_SKY_COLOR),
    clearColorScale: parseFloatParam(url, "clearColorScale", DEFAULT_CLEAR_COLOR_SCALE, 0.0, 1.0),
  };
}
