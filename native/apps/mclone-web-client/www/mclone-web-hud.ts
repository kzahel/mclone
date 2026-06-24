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
  setHudOpen(runtimeState, defaultHudOpen());
  setMenuOpen(app, runtimeState, false);
  setSettingsOpen(runtimeState, false);
  syncSettingsControls(app);

  app.hudToggle?.addEventListener("click", () => {
    if (runtimeState.uiActive === true) {
      app.closeNativeUi();
    } else {
      app.openNativePauseUi();
    }
  });

  const lookInput = document.getElementById("setting-look-sensitivity") as HTMLInputElement | null;
  lookInput?.addEventListener("input", () => {
    setLookSensitivity(app, runtimeState, Number(lookInput.value));
  });

  updateMenuShortcutState(runtimeState);
}

export function setMenuOpen(app: HudMenuApp, runtimeState: HudRuntimeState, open: boolean): void {
  const menu = document.getElementById("main-menu");
  const toggle = document.getElementById("hud-toggle");
  if (menu) {
    menu.hidden = true;
  }
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
  const panel = document.getElementById("settings-panel");
  const button = document.getElementById("menu-settings");
  if (panel) {
    panel.hidden = !open;
  }
  if (button) {
    button.setAttribute("aria-expanded", open ? "true" : "false");
  }
  runtimeState.settingsOpen = Boolean(open);
}

function setLookSensitivity(app: HudMenuApp, runtimeState: HudRuntimeState, value: number): void {
  const clamped = clampLookSensitivity(value);
  app.lookSensitivity = clamped;
  runtimeState.lookSensitivity = clamped;
  storeSetting(SETTINGS_STORAGE_KEYS.lookSensitivity, String(clamped));
  syncSettingsControls(app);
}

function syncSettingsControls(app: HudMenuApp): void {
  const lookInput = document.getElementById("setting-look-sensitivity") as HTMLInputElement | null;
  if (lookInput && document.activeElement !== lookInput) {
    lookInput.value = String(app.lookSensitivity);
  }
  const lookValue = document.getElementById("setting-look-sensitivity-value");
  if (lookValue) {
    lookValue.textContent = `${app.lookSensitivity.toFixed(1)}×`;
  }
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

function storeSetting(key: string, value: string): void {
  try {
    globalThis.localStorage?.setItem(key, value);
  } catch (_error) {
    // Private browsing / disabled storage: keep the in-memory setting only.
  }
}

export function updateDom(state: HudRuntimeState): void {
  if (state.failed || (state.ready && !state.ok)) {
    setHudOpen(state, true);
  }
  updateMenuShortcutState(state);
  setText("center", `${state.centerX}, ${state.centerZ}`);
  setText("camera", `${state.cameraX.toFixed(1)}, ${state.cameraY.toFixed(1)}, ${state.cameraZ.toFixed(1)}`);
  setText("mode", state.movementMode);
  setText("slot", String(Number(state.selectedHotbarSlot || 0) + 1));
  setText("ground", state.onGround ? "ground" : state.verticalCollision ? "blocked" : state.horizontalCollision ? "wall" : "air");
  setText("target", formatTarget(state.currentTarget));
  setText("action", state.interactionStatus);
  setText("chunks", String(state.loadedChunkCount));
  setText("sections", String(state.residentSectionCount));
  setText("time", `${Number(state.dayTime || 0).toFixed(0)} / ${Number(state.timeOfDay || 0).toFixed(3)}`);
  setText("actors", `${state.drawnActorCount}/${state.actorCount}`);
  setText("pending", String(state.pendingCompileJobCount));
  setText("compile", formatCompileTiming(state));
  setText("frames", String(state.frameCount));
  setText("lock", state.pointerLocked ? "on" : state.pointerLockFallback ? "fallback" : "off");
  const status = document.getElementById("status");
  if (status) {
    status.textContent = state.status;
    status.dataset.ok = state.ok ? "true" : "false";
    status.dataset.ready = state.ready && state.status === "ready" ? "true" : "false";
  }
}

function formatCompileTiming(state: HudRuntimeState): string {
  const timing = state.activeCompileTiming ?? state.lastCompileTiming;
  if (!timing) {
    return "-";
  }
  const label = state.activeCompileTiming ? "active" : timing.status;
  const target = `${timing.targetCenterX},${timing.targetCenterZ}`;
  return `${label} ${target} ${Number(timing.totalMs || 0).toFixed(0)}ms gap ${Number(timing.maxFrameGapMs || 0).toFixed(0)}ms`;
}

function setText(id: string, value: string): void {
  const element = document.getElementById(id);
  if (element) element.textContent = value;
}

function updateMenuShortcutState(state: HudRuntimeState): void {
  const menu = document.getElementById("main-menu");
  if (menu) {
    menu.hidden = true;
  }
  const toggle = document.getElementById("hud-toggle");
  if (toggle) {
    toggle.setAttribute("aria-expanded", state.uiActive === true ? "true" : "false");
  }
  state.menuOpen = false;
}

export function setHudOpen(runtimeState: HudRuntimeState, open: boolean): void {
  const hud = document.getElementById("runtime-hud");
  const toggle = document.getElementById("hud-toggle");
  if (hud) {
    hud.hidden = !open;
  }
  if (toggle) {
    toggle.setAttribute("aria-expanded", open ? "true" : "false");
  }
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

function formatTarget(interaction: WasmReport | null | undefined): string {
  if (!interaction?.ok) {
    return "-";
  }
  if (!interaction.hit) {
    return "miss";
  }
  return `${interaction.blockX}, ${interaction.blockY}, ${interaction.blockZ}`;
}
