// Raw browser keyboard/mouse glue. Deploy note: mclone-web-app.js is
// cache-busted with `?v=<version>`, but a static `import` of this module cannot
// carry that query string, so a deploy that bumps the asset version must rely on
// HTTP cache revalidation of this bare URL (smoke/dev leave the version unset).

import type { TouchControls } from "./mclone-web-touch.js";

export interface PointerDownState {
  button: number;
  enabled: boolean;
  movement: number;
}

export interface InputBindingApp {
  canvas: HTMLCanvasElement;
  touchControls: TouchControls | null;
  pointerDragging: boolean;
  pointerDown: PointerDownState | null;
  handleRawKey(code: string, key: string, pressed: boolean, repeat: boolean): boolean;
  handleRawPointerButton(
    button: number,
    pressed: boolean,
    click: boolean,
    clientX: number,
    clientY: number,
  ): boolean;
  handleRawPointerMove(clientX: number, clientY: number): boolean;
  handleRawMouseMotion(dx: number, dy: number): boolean;
  handleRawWheel(deltaY: number, deltaMode: number): boolean;
  clearRawInput(): void;
  requestPointerLock(): void;
  updatePointerLockState(): void;
  syncCanvasSize(): void;
}

interface InputRuntimeState extends Record<string, any> {
  pointerLockAttempted?: boolean;
  pointerLockFallback?: boolean;
}

export function bindInput(
  app: InputBindingApp,
  runtimeState: InputRuntimeState,
  publishRuntimeState: () => void,
): void {
  window.addEventListener("keydown", (event) => {
    if (app.handleRawKey(keyboardCode(event), event.key, true, event.repeat)) {
      event.preventDefault();
    }
  });

  window.addEventListener("keyup", (event) => {
    if (app.handleRawKey(keyboardCode(event), event.key, false, false)) {
      event.preventDefault();
    }
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
      enabled: Boolean(runtimeState.pointerLockAttempted),
      movement: 0,
    };
    app.canvas.focus();
    if (app.handleRawPointerButton(
      event.button,
      true,
      false,
      event.clientX,
      event.clientY,
    ) || event.button === 2) {
      event.preventDefault();
    }
  });

  window.addEventListener("mouseup", (event) => {
    if (app.touchControls?.shouldIgnoreMouseEvent()) {
      app.pointerDragging = false;
      app.pointerDown = null;
      event.preventDefault();
      return;
    }
    const pointerDown = app.pointerDown;
    const click = Boolean(
      pointerDown?.enabled
      && pointerDown.button === event.button
      && pointerDown.movement <= 4,
    );
    app.pointerDragging = false;
    app.pointerDown = null;
    if (app.handleRawPointerButton(
      event.button,
      false,
      click,
      event.clientX,
      event.clientY,
    )) {
      event.preventDefault();
    }
  });

  app.canvas.addEventListener("contextmenu", (event) => {
    event.preventDefault();
  });

  window.addEventListener("mousemove", (event) => {
    if (app.touchControls?.shouldIgnoreMouseEvent()) {
      return;
    }
    app.handleRawPointerMove(event.clientX, event.clientY);
    if (document.pointerLockElement === app.canvas || app.pointerDragging) {
      app.handleRawMouseMotion(event.movementX, event.movementY);
      if (app.pointerDown) {
        app.pointerDown.movement += Math.abs(finiteNumber(event.movementX))
          + Math.abs(finiteNumber(event.movementY));
      }
    }
  });

  app.canvas.addEventListener("wheel", (event) => {
    if (app.handleRawWheel(event.deltaY, event.deltaMode)) {
      event.preventDefault();
    }
  }, { passive: false });

  window.addEventListener("blur", () => app.clearRawInput());
  document.addEventListener("pointerlockchange", () => app.updatePointerLockState());
  document.addEventListener("pointerlockerror", () => {
    runtimeState.pointerLockFallback = true;
    publishRuntimeState();
  });
  window.addEventListener("resize", () => {
    app.syncCanvasSize();
    publishRuntimeState();
  });
}

function keyboardCode(event: KeyboardEvent): string {
  return typeof event.code === "string" && event.code !== "Unidentified"
    ? event.code
    : "";
}

function finiteNumber(value: unknown): number {
  const number = Number(value);
  return Number.isFinite(number) ? number : 0;
}
