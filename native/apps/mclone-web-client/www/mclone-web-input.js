// Native web keyboard/mouse input glue. Deploy note: mclone-web-app.js is
// cache-busted with `?v=<version>`, but a static `import` of this module cannot
// carry that query string, so a deploy that bumps the asset version must rely on
// HTTP cache revalidation of this bare URL (smoke/dev leave the version unset).

export const INPUT_KEY_NAMES = ["forward", "backward", "left", "right", "jump", "descend", "shift", "sprint"];

/**
 * @typedef {import("./mclone-web-touch.js").TouchControls} TouchControls
 * @typedef {Record<string, any>} WasmReport
 */

/**
 * @typedef {object} PointerDownState
 * @property {number} button
 * @property {boolean} enabled
 * @property {number} movement
 */

/**
 * @typedef {object} InputBindingApp
 * @property {HTMLCanvasElement} canvas
 * @property {WasmReport | null} session
 * @property {TouchControls | null} touchControls
 * @property {boolean} pointerDragging
 * @property {PointerDownState | null} pointerDown
 * @property {(camera: WasmReport) => void} applyCameraState
 * @property {(slot: number) => boolean} selectHotbarSlot
 * @property {(name: string, down: boolean) => boolean} setInputKey
 * @property {(dx: number, dy: number) => void} queueMouseDelta
 * @property {(action: string) => Promise<any>} interactBlock
 * @property {() => void} requestPointerLock
 * @property {() => void} updatePointerLockState
 * @property {() => void} syncCanvasSize
 */

/**
 * @param {InputBindingApp} app
 * @param {Record<string, any>} runtimeState
 * @param {() => void} updateDom
 */
export function bindInput(app, runtimeState, updateDom) {
  window.addEventListener("keydown", (event) => {
    if (isPhysicalKey(event, "KeyN", "n") && !event.repeat) {
      event.preventDefault();
      const camera = app.session?.toggleMovementMode?.();
      if (camera) app.applyCameraState(camera);
      updateDom();
      return;
    }
    const hotbarSlot = hotbarSlotForEvent(event);
    if (hotbarSlot !== null) {
      event.preventDefault();
      if (!event.repeat) {
        app.selectHotbarSlot(hotbarSlot);
      }
      return;
    }
    const key = inputNameForEvent(event);
    if (!key) return;
    event.preventDefault();
    app.setInputKey(key, true);
  });

  window.addEventListener("keyup", (event) => {
    const key = inputNameForEvent(event);
    if (!key) return;
    event.preventDefault();
    app.setInputKey(key, false);
  });

  app.canvas.addEventListener("click", (event) => {
    if (app.touchControls?.shouldIgnoreMouseEvent()) {
      event.preventDefault();
      return;
    }
    app.canvas.focus();
    app.requestPointerLock();
  });

  app.canvas.addEventListener("mousedown", (event) => {
    if (app.touchControls?.shouldIgnoreMouseEvent()) {
      event.preventDefault();
      return;
    }
    app.pointerDragging = true;
    app.pointerDown = {
      button: event.button,
      enabled: runtimeState.pointerLockAttempted,
      movement: 0,
    };
    app.canvas.focus();
    if (event.button === 2) {
      event.preventDefault();
    }
  });

  window.addEventListener("mouseup", (event) => {
    const pointerDown = app.pointerDown;
    app.pointerDragging = false;
    app.pointerDown = null;
    if (!pointerDown?.enabled || pointerDown.button !== event.button || pointerDown.movement > 4) {
      return;
    }
    if (event.button === 0) {
      event.preventDefault();
      app.interactBlock("break");
    } else if (event.button === 2) {
      event.preventDefault();
      app.interactBlock("place");
    }
  });

  app.canvas.addEventListener("contextmenu", (event) => {
    event.preventDefault();
  });

  window.addEventListener("mousemove", (event) => {
    if (app.touchControls?.shouldIgnoreMouseEvent()) {
      return;
    }
    if (document.pointerLockElement === app.canvas) {
      app.queueMouseDelta(event.movementX, event.movementY);
    } else if (app.pointerDragging) {
      app.queueMouseDelta(event.movementX, event.movementY);
    }
  });

  app.canvas.addEventListener("wheel", (event) => {
    event.preventDefault();
    const amount = event.deltaMode === WheelEvent.DOM_DELTA_PIXEL
      ? -event.deltaY * 0.001
      : -event.deltaY * 0.12;
    const camera = app.session?.adjustCameraSpeed?.(amount);
    if (camera) app.applyCameraState(camera);
    updateDom();
  }, { passive: false });

  document.addEventListener("pointerlockchange", () => app.updatePointerLockState());
  document.addEventListener("pointerlockerror", () => {
    runtimeState.pointerLockFallback = true;
    updateDom();
  });
  window.addEventListener("resize", () => {
    app.syncCanvasSize();
    updateDom();
  });
}

/** @param {KeyboardEvent} event */
function inputNameForEvent(event) {
  const code = keyboardCode(event);
  switch (code) {
    case "ArrowUp":
    case "KeyW":
      return "forward";
    case "ArrowDown":
    case "KeyS":
      return "backward";
    case "ArrowLeft":
    case "KeyA":
      return "left";
    case "ArrowRight":
    case "KeyD":
      return "right";
    case "Space":
      return "jump";
    case "KeyX":
    case "KeyQ":
      return "descend";
    case "ShiftLeft":
    case "ShiftRight":
      return "shift";
    case "ControlLeft":
    case "ControlRight":
      return "sprint";
    default:
      return code === null ? inputNameForLegacyKey(event.key) : null;
  }
}

/** @param {string} key */
function inputNameForLegacyKey(key) {
  switch (key) {
    case "ArrowUp":
    case "w":
    case "W":
      return "forward";
    case "ArrowDown":
    case "s":
    case "S":
      return "backward";
    case "ArrowLeft":
    case "a":
    case "A":
      return "left";
    case "ArrowRight":
    case "d":
    case "D":
      return "right";
    case " ":
    case "Spacebar":
      return "jump";
    case "x":
    case "X":
    case "q":
    case "Q":
      return "descend";
    case "Shift":
      return "shift";
    case "Control":
      return "sprint";
    default:
      return null;
  }
}

/**
 * @param {KeyboardEvent} event
 * @param {string} code
 * @param {string} legacyKey
 */
function isPhysicalKey(event, code, legacyKey) {
  if (keyboardCode(event) === code) {
    return true;
  }
  return keyboardCode(event) === null && (
    event.key === legacyKey || event.key === legacyKey.toUpperCase()
  );
}

/** @param {KeyboardEvent} event */
function keyboardCode(event) {
  return typeof event.code === "string" && event.code.length > 0 && event.code !== "Unidentified"
    ? event.code
    : null;
}

export function defaultInputKeys() {
  return Object.fromEntries(INPUT_KEY_NAMES.map((name) => [name, false]));
}

export function defaultMovementImpulse() {
  return {
    active: false,
    left: 0,
    forward: 0,
  };
}

/** @param {unknown} value */
export function sanitizeInputImpulse(value) {
  const impulse = Number(value);
  if (!Number.isFinite(impulse)) {
    return 0;
  }
  return Math.max(-1, Math.min(1, impulse));
}

/**
 * @param {Record<string, any> | null | undefined} value
 * @param {Record<string, any>} runtimeState
 */
export function applyHotbarState(value, runtimeState) {
  if (!value || typeof value.selectedHotbarSlot === "undefined") {
    return;
  }
  const slot = Number(value.selectedHotbarSlot);
  if (Number.isInteger(slot) && slot >= 0 && slot < 9) {
    runtimeState.selectedHotbarSlot = slot;
  }
}

/** @param {KeyboardEvent} event */
function hotbarSlotForEvent(event) {
  const code = keyboardCode(event);
  if (code?.startsWith("Digit") || code?.startsWith("Numpad")) {
    const digit = Number(code.slice(-1));
    if (Number.isInteger(digit) && digit >= 1 && digit <= 9) {
      return digit - 1;
    }
    return null;
  }
  return code === null ? hotbarSlotForLegacyKey(event.key) : null;
}

/** @param {string} key */
function hotbarSlotForLegacyKey(key) {
  if (key.length !== 1) {
    return null;
  }
  const digit = Number(key);
  if (Number.isInteger(digit) && digit >= 1 && digit <= 9) {
    return digit - 1;
  }
  return null;
}
