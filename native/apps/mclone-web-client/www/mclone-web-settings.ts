// Native web platform settings. Deploy note: mclone-web-app.js is
// cache-busted with `?v=<version>`, but a static `import` of this module cannot
// carry that query string, so a deploy that bumps the asset version must rely
// on HTTP cache revalidation of this bare URL (smoke/dev leave the version
// unset).

// Touch look multiplies the raw drag delta before it reaches the engine's fixed
// mouse sensitivity. A finger drag covers far fewer pixels than a relative mouse
// move (especially on the narrow look-half of a portrait phone), so the default
// boost makes aiming the crosshair toward your travel direction practical.
export const LOOK_SENSITIVITY_MIN = 0.5;
export const LOOK_SENSITIVITY_MAX = 5;
export const DEFAULT_LOOK_SENSITIVITY = 2.4;

const SETTINGS_STORAGE_KEYS = {
  lookSensitivity: "mclone.web.lookSensitivity",
  touchControlsMode: "mclone.web.touchControlsMode",
};

type WasmReport = Record<string, any>;
export type TouchControlsMode = "auto" | "on" | "off";

export interface StoredWebSettings {
  lookSensitivity: number;
  touchControlsMode: TouchControlsMode;
}

export function clampLookSensitivity(value: unknown): number {
  const sensitivity = Number(value);
  if (!Number.isFinite(sensitivity)) {
    return DEFAULT_LOOK_SENSITIVITY;
  }
  return Math.min(LOOK_SENSITIVITY_MAX, Math.max(LOOK_SENSITIVITY_MIN, sensitivity));
}

export function loadStoredSettings(): StoredWebSettings {
  return {
    lookSensitivity: clampLookSensitivity(
      readStoredSetting(SETTINGS_STORAGE_KEYS.lookSensitivity) ?? DEFAULT_LOOK_SENSITIVITY,
    ),
    touchControlsMode: parseTouchControlsMode(
      readStoredSetting(SETTINGS_STORAGE_KEYS.touchControlsMode),
    ),
  };
}

export function storeLookSensitivity(value: number): void {
  storeSetting(SETTINGS_STORAGE_KEYS.lookSensitivity, String(clampLookSensitivity(value)));
}

export function parseTouchControlsMode(value: unknown): TouchControlsMode {
  return value === "on" || value === "off" ? value : "auto";
}

export function storeTouchControlsMode(value: TouchControlsMode): void {
  storeSetting(SETTINGS_STORAGE_KEYS.touchControlsMode, parseTouchControlsMode(value));
}

function readStoredSetting(key: string): string | null {
  try {
    return globalThis.localStorage?.getItem(key) ?? null;
  } catch (_error) {
    return null;
  }
}

function storeSetting(key: string, value: string): void {
  try {
    globalThis.localStorage?.setItem(key, value);
  } catch (_error) {
    // Storage can be disabled in private or embedded browser contexts.
  }
}

export function formatInteractionStatus(interaction: WasmReport | null | undefined): string {
  if (!interaction?.ok) {
    return "idle";
  }
  if (!interaction.hit) {
    return `${interaction.action}: miss`;
  }
  return `${interaction.action}: ${interaction.changed ? "changed" : "same"}`;
}
