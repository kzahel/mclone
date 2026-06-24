import { hasTouchInput } from "./mclone-web-touch.js";
import type { TouchControls } from "./mclone-web-touch.js";

// Native web HUD/menu/settings glue. Deploy note: mclone-web-app.js is
// cache-busted with `?v=<version>`, but a static `import` of this module cannot
// carry that query string, so a deploy that bumps the asset version must rely on
// HTTP cache revalidation of this bare URL (smoke/dev leave the version unset).

// Touch look multiplies the raw drag delta before it reaches the engine's fixed
// mouse sensitivity. A finger drag covers far fewer pixels than a relative mouse
// move (especially on the narrow look-half of a portrait phone), so the default
// boost makes aiming the crosshair toward your travel direction practical.
const LOOK_SENSITIVITY_MIN = 0.5;
const LOOK_SENSITIVITY_MAX = 5;
export const DEFAULT_LOOK_SENSITIVITY = 2.4;
const SETTINGS_STORAGE_KEYS = {
  lookSensitivity: "mclone.web.lookSensitivity",
};

type WasmReport = Record<string, any>;

export interface HudMenuApp {
  canvas: HTMLCanvasElement;
  hudToggle: Element | null;
  lookSensitivity: number;
  touchControls: TouchControls | null;
  openNativePauseUi(): WasmReport | null;
  closeNativeUi(): WasmReport | null;
  setNativeDebugOverlay(open: boolean): WasmReport | null;
}

interface HudRuntimeState extends Record<string, any> {
  failed?: boolean;
  ready?: boolean;
  ok?: boolean;
  hudOpen?: boolean;
  menuOpen?: boolean;
  settingsOpen?: boolean;
  lookSensitivity?: number;
}

export interface StoredHudSettings {
  lookSensitivity: number;
}

export function bindMenu(app: HudMenuApp, runtimeState: HudRuntimeState): void {
  // DOM menu controls are retired as authoritative UI. The hamburger remains as
  // a browser/touch shortcut that opens the native-rendered pause screen.
  app.setNativeDebugOverlay(defaultHudOpen());
  setMenuOpen(app, runtimeState, false);
  setSettingsOpen(runtimeState, false);

  app.hudToggle?.addEventListener("click", () => {
    if (runtimeState.uiActive === true) {
      app.closeNativeUi();
    } else {
      app.openNativePauseUi();
    }
  });

  updateMenuShortcutState(runtimeState);
}

export function setMenuOpen(app: HudMenuApp, runtimeState: HudRuntimeState, open: boolean): void {
  setSettingsOpen(runtimeState, false);
  if (open) {
    app?.touchControls?.clearAll();
    app.openNativePauseUi();
  } else {
    app.closeNativeUi();
  }
  runtimeState.menuOpen = false;
  updateMenuShortcutState(runtimeState);
}

function setSettingsOpen(runtimeState: HudRuntimeState, open: boolean): void {
  runtimeState.settingsOpen = Boolean(open);
}

function clampLookSensitivity(value: unknown): number {
  const sensitivity = Number(value);
  if (!Number.isFinite(sensitivity)) {
    return DEFAULT_LOOK_SENSITIVITY;
  }
  return Math.min(LOOK_SENSITIVITY_MAX, Math.max(LOOK_SENSITIVITY_MIN, sensitivity));
}

export function loadStoredSettings(): StoredHudSettings {
  return {
    lookSensitivity: clampLookSensitivity(
      readStoredSetting(SETTINGS_STORAGE_KEYS.lookSensitivity) ?? DEFAULT_LOOK_SENSITIVITY,
    ),
  };
}

function readStoredSetting(key: string): string | null {
  try {
    return globalThis.localStorage?.getItem(key) ?? null;
  } catch (_error) {
    return null;
  }
}

export function updateDom(state: HudRuntimeState): void {
  if (state.failed || (state.ready && !state.ok)) {
    setHudOpen(state, true);
  }
  updateMenuShortcutState(state);
}

function updateMenuShortcutState(state: HudRuntimeState): void {
  const toggle = document.getElementById("hud-toggle");
  if (toggle) {
    toggle.setAttribute("aria-expanded", state.uiActive === true ? "true" : "false");
  }
  state.menuOpen = false;
}

export function setHudOpen(runtimeState: HudRuntimeState, open: boolean): void {
  runtimeState.hudOpen = Boolean(open);
}

function defaultHudOpen(): boolean {
  return !hasTouchInput() && window.matchMedia("(min-width: 681px)").matches;
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
