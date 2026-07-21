// Raw browser touch/pen glue. Control layout, hit testing, bindings, held
// state, dead zones, sensitivity application, and overlay meaning live in
// shared Rust. This module retains DOM pointer capture and synthetic-mouse
// suppression only.

export interface TouchControlApp {
  canvas: HTMLCanvasElement;
  handleRawTouch(
    phase: "start" | "move" | "end" | "cancel",
    pointerId: number,
    clientX: number,
    clientY: number,
  ): boolean;
  setTouchInputAvailable(available: boolean): Record<string, any> | null;
}

export interface TouchControlSnapshot {
  activePointerCount: number;
}

export class TouchControls {
  private readonly app: TouchControlApp;
  readonly canvas: HTMLCanvasElement;
  private readonly activePointerIds: Set<number>;
  private lastTouchAt: number;

  constructor(app: TouchControlApp) {
    this.app = app;
    this.canvas = app.canvas;
    this.activePointerIds = new Set();
    this.lastTouchAt = 0;

    this.app.setTouchInputAvailable(hasTouchInput());
    this.bindCanvas();
    window.addEventListener("blur", () => this.clearAll());
  }

  private bindCanvas(): void {
    this.canvas.addEventListener(
      "pointerdown",
      (event) => this.forwardPointer(event, "start"),
      { passive: false },
    );
    this.canvas.addEventListener(
      "pointermove",
      (event) => this.forwardPointer(event, "move"),
      { passive: false },
    );
    this.canvas.addEventListener(
      "pointerup",
      (event) => this.forwardPointer(event, "end"),
      { passive: false },
    );
    this.canvas.addEventListener(
      "pointercancel",
      (event) => this.forwardPointer(event, "cancel"),
      { passive: false },
    );
    this.canvas.addEventListener(
      "lostpointercapture",
      (event) => this.forwardPointer(event, "cancel"),
      { passive: false },
    );
  }

  private forwardPointer(
    event: PointerEvent,
    phase: "start" | "move" | "end" | "cancel",
  ): void {
    if (!isTouchPointer(event)) {
      return;
    }
    if (phase !== "start" && !this.activePointerIds.has(event.pointerId)) {
      return;
    }
    this.markTouchEvent();
    event.preventDefault();
    this.app.setTouchInputAvailable(true);
    this.canvas.focus();
    if (phase === "start") {
      this.activePointerIds.add(event.pointerId);
      trySetPointerCapture(this.canvas, event.pointerId);
    }
    this.app.handleRawTouch(
      phase,
      event.pointerId,
      event.clientX,
      event.clientY,
    );
    if (phase === "end" || phase === "cancel") {
      this.activePointerIds.delete(event.pointerId);
    }
  }

  clearAll(): void {
    this.activePointerIds.clear();
  }

  private markTouchEvent(): void {
    this.lastTouchAt = performance.now();
  }

  shouldIgnoreMouseEvent(): boolean {
    return performance.now() - this.lastTouchAt < 800;
  }

  snapshot(): TouchControlSnapshot {
    return {
      activePointerCount: this.activePointerIds.size,
    };
  }
}

export function hasTouchInput(): boolean {
  return (typeof navigator !== "undefined" && navigator.maxTouchPoints > 0)
    || window.matchMedia("(pointer: coarse)").matches;
}

function isTouchPointer(event: PointerEvent): boolean {
  return event.pointerType === "touch" || event.pointerType === "pen";
}

function trySetPointerCapture(target: EventTarget | null, pointerId: number): void {
  if (!(target instanceof HTMLElement) || typeof target.setPointerCapture !== "function") {
    return;
  }
  try {
    target.setPointerCapture(pointerId);
  } catch (_error) {
    // Some synthetic or browser-generated pointer streams cannot be captured.
  }
}
