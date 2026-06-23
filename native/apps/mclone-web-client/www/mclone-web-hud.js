import { hasTouchInput } from "./mclone-web-touch.js";

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

/**
 * @typedef {import("./mclone-web-touch.js").TouchControls} TouchControls
 * @typedef {Record<string, any>} WasmReport
 */

/**
 * @typedef {object} HudMenuApp
 * @property {HTMLCanvasElement} canvas
 * @property {Element | null} hudToggle
 * @property {number} lookSensitivity
 * @property {TouchControls | null} touchControls
 */

/**
 * @param {HudMenuApp} app
 * @param {Record<string, any>} runtimeState
 */
export function bindMenu(app, runtimeState) {
  // The hamburger now opens the main menu; the runtime/debug stats live behind
  // the menu's "Debug info" entry. Stats stay open by default on desktop so the
  // debug HUD remains a glance away, while mobile boots into the clean view.
  setHudOpen(runtimeState, defaultHudOpen());
  setMenuOpen(app, runtimeState, false);
  setSettingsOpen(runtimeState, false);
  syncSettingsControls(app);

  app.hudToggle?.addEventListener("click", () => setMenuOpen(app, runtimeState, !isMenuOpen()));

  document.getElementById("menu-resume")?.addEventListener("click", () => {
    setMenuOpen(app, runtimeState, false);
  });

  const debugButton = document.getElementById("menu-debug");
  debugButton?.addEventListener("click", () => {
    const open = !isHudOpen();
    setHudOpen(runtimeState, open);
    setMenuOpen(app, runtimeState, false);
  });

  document.getElementById("menu-settings")?.addEventListener("click", () => {
    setSettingsOpen(runtimeState, !isSettingsOpen());
  });

  const lookInput = /** @type {HTMLInputElement | null} */ (
    document.getElementById("setting-look-sensitivity")
  );
  lookInput?.addEventListener("input", () => {
    setLookSensitivity(app, runtimeState, Number(lookInput.value));
  });

  window.addEventListener("keydown", (event) => {
    if (event.key !== "Escape" || document.pointerLockElement === app.canvas) {
      return;
    }
    if (isSettingsOpen()) {
      setSettingsOpen(runtimeState, false);
    } else if (isMenuOpen()) {
      setMenuOpen(app, runtimeState, false);
    } else if (isHudOpen()) {
      setHudOpen(runtimeState, false);
    }
  });
}

/**
 * @param {HudMenuApp} app
 * @param {Record<string, any>} runtimeState
 * @param {boolean} open
 */
export function setMenuOpen(app, runtimeState, open) {
  const menu = document.getElementById("main-menu");
  const toggle = document.getElementById("hud-toggle");
  if (menu) {
    menu.hidden = !open;
  }
  if (toggle) {
    toggle.setAttribute("aria-expanded", open ? "true" : "false");
  }
  if (!open) {
    setSettingsOpen(runtimeState, false);
  } else {
    // Releasing held touch input keeps the player from drifting while the modal
    // menu is consuming the screen.
    app?.touchControls?.clearAll();
    syncSettingsControls(app);
  }
  const debugButton = document.getElementById("menu-debug");
  debugButton?.setAttribute("aria-pressed", isHudOpen() ? "true" : "false");
  runtimeState.menuOpen = Boolean(open);
}

function isMenuOpen() {
  return document.getElementById("main-menu")?.hidden === false;
}

/**
 * @param {Record<string, any>} runtimeState
 * @param {boolean} open
 */
function setSettingsOpen(runtimeState, open) {
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

function isSettingsOpen() {
  return document.getElementById("settings-panel")?.hidden === false;
}

/**
 * @param {HudMenuApp} app
 * @param {Record<string, any>} runtimeState
 * @param {number} value
 */
function setLookSensitivity(app, runtimeState, value) {
  const clamped = clampLookSensitivity(value);
  app.lookSensitivity = clamped;
  runtimeState.lookSensitivity = clamped;
  storeSetting(SETTINGS_STORAGE_KEYS.lookSensitivity, String(clamped));
  syncSettingsControls(app);
}

/** @param {HudMenuApp} app */
function syncSettingsControls(app) {
  const lookInput = /** @type {HTMLInputElement | null} */ (
    document.getElementById("setting-look-sensitivity")
  );
  if (lookInput && document.activeElement !== lookInput) {
    lookInput.value = String(app.lookSensitivity);
  }
  const lookValue = document.getElementById("setting-look-sensitivity-value");
  if (lookValue) {
    lookValue.textContent = `${app.lookSensitivity.toFixed(1)}×`;
  }
}

/** @param {unknown} value */
function clampLookSensitivity(value) {
  const sensitivity = Number(value);
  if (!Number.isFinite(sensitivity)) {
    return DEFAULT_LOOK_SENSITIVITY;
  }
  return Math.min(LOOK_SENSITIVITY_MAX, Math.max(LOOK_SENSITIVITY_MIN, sensitivity));
}

export function loadStoredSettings() {
  return {
    lookSensitivity: clampLookSensitivity(
      readStoredSetting(SETTINGS_STORAGE_KEYS.lookSensitivity) ?? DEFAULT_LOOK_SENSITIVITY,
    ),
  };
}

/** @param {string} key */
function readStoredSetting(key) {
  try {
    return globalThis.localStorage?.getItem(key) ?? null;
  } catch (_error) {
    return null;
  }
}

/**
 * @param {string} key
 * @param {string} value
 */
function storeSetting(key, value) {
  try {
    globalThis.localStorage?.setItem(key, value);
  } catch (_error) {
    // Private browsing / disabled storage: keep the in-memory setting only.
  }
}

/** @param {Record<string, any>} state */
export function updateDom(state) {
  if (state.failed || (state.ready && !state.ok)) {
    setHudOpen(state, true);
  }
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

/** @param {Record<string, any>} state */
function formatCompileTiming(state) {
  const timing = state.activeCompileTiming ?? state.lastCompileTiming;
  if (!timing) {
    return "-";
  }
  const label = state.activeCompileTiming ? "active" : timing.status;
  const target = `${timing.targetCenterX},${timing.targetCenterZ}`;
  return `${label} ${target} ${Number(timing.totalMs || 0).toFixed(0)}ms gap ${Number(timing.maxFrameGapMs || 0).toFixed(0)}ms`;
}

/**
 * @param {string} id
 * @param {string} value
 */
function setText(id, value) {
  const element = document.getElementById(id);
  if (element) element.textContent = value;
}

function isHudOpen() {
  return document.getElementById("runtime-hud")?.hidden === false;
}

/**
 * @param {Record<string, any>} runtimeState
 * @param {boolean} open
 */
export function setHudOpen(runtimeState, open) {
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

function defaultHudOpen() {
  return !hasTouchInput() && window.matchMedia("(min-width: 681px)").matches;
}

/** @param {WasmReport} interaction */
export function formatInteractionStatus(interaction) {
  if (!interaction?.ok) {
    return "idle";
  }
  if (!interaction.hit) {
    return `${interaction.action}: miss`;
  }
  return `${interaction.action}: ${interaction.changed ? "changed" : "same"}`;
}

/** @param {WasmReport} interaction */
function formatTarget(interaction) {
  if (!interaction?.ok) {
    return "-";
  }
  if (!interaction.hit) {
    return "miss";
  }
  return `${interaction.blockX}, ${interaction.blockY}, ${interaction.blockZ}`;
}
