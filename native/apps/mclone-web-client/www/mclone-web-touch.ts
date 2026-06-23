// Native web touch-control glue. Deploy note: mclone-web-app.js is cache-busted
// with `?v=<version>`, but a static `import` of this module cannot carry that
// query string, so a deploy that bumps the asset version must rely on HTTP cache
// revalidation of this bare URL (smoke/dev leave the version unset).

const TOUCH_JOYSTICK_MAX_DISTANCE = 50;
const TOUCH_JOYSTICK_DEAD_ZONE = 10;
const TOUCH_AXIS_THRESHOLD = TOUCH_JOYSTICK_DEAD_ZONE / TOUCH_JOYSTICK_MAX_DISTANCE;

export interface TouchMovementImpulse {
  active: boolean;
  left: number;
  forward: number;
}

export interface TouchControlApp {
  canvas: HTMLCanvasElement;
  lookSensitivity: number;
  touchMovementImpulse: TouchMovementImpulse;
  touchKeys: Record<string, boolean>;
  setTouchKey(name: string, down: boolean): boolean;
  setTouchKeys(keys: Record<string, boolean>): void;
  setTouchMovementImpulse(left: number, forward: number, active: boolean): void;
  queueMouseDelta(dx: number, dy: number): void;
  currentMovementImpulse(): TouchMovementImpulse;
}

interface TouchRuntimeState extends Record<string, any> {
  touchControlsVisible?: boolean;
  touchJoystickActive?: boolean;
  touchMovementLeftImpulse?: number;
  touchMovementForwardImpulse?: number;
  touchLookActive?: boolean;
  touchButtonActiveCount?: number;
}

export interface TouchControlSnapshot {
  visible: boolean;
  joystickActive: boolean;
  lookActive: boolean;
  buttonActiveCount: number;
  keys: Record<string, boolean>;
  movementImpulse: TouchMovementImpulse;
}

export class TouchControls {
  private readonly app: TouchControlApp;
  private readonly runtimeState: TouchRuntimeState;
  readonly canvas: HTMLCanvasElement;
  private readonly root: HTMLElement | null;
  private readonly joystick: HTMLElement | null;
  private readonly thumb: HTMLElement | null;
  private readonly buttons: HTMLElement[];
  private movementPointerId: number | null;
  private lookPointerId: number | null;
  private movementBaseX: number;
  private movementBaseY: number;
  private movementThumbX: number;
  private movementThumbY: number;
  private lookLastX: number;
  private lookLastY: number;
  private readonly buttonPointers: Map<number, string>;
  private lastTouchAt: number;

  constructor(app: TouchControlApp, runtimeState: TouchRuntimeState) {
    this.app = app;
    this.runtimeState = runtimeState;
    this.canvas = app.canvas;
    this.root = document.getElementById("touch-controls");
    this.joystick = document.getElementById("touch-joystick");
    this.thumb = document.getElementById("touch-joystick-thumb");
    this.buttons = Array.from(document.querySelectorAll<HTMLElement>("[data-touch-key]"));
    this.movementPointerId = null;
    this.lookPointerId = null;
    this.movementBaseX = 0;
    this.movementBaseY = 0;
    this.movementThumbX = 0;
    this.movementThumbY = 0;
    this.lookLastX = 0;
    this.lookLastY = 0;
    this.buttonPointers = new Map();
    this.lastTouchAt = 0;

    this.setVisible(hasTouchInput());
    this.bindCanvas();
    this.bindButtons();
    window.addEventListener("blur", () => this.clearAll());
  }

  private bindCanvas(): void {
    this.canvas.addEventListener("pointerdown", (event) => this.onCanvasPointerDown(event), { passive: false });
    this.canvas.addEventListener("pointermove", (event) => this.onCanvasPointerMove(event), { passive: false });
    this.canvas.addEventListener("pointerup", (event) => this.onCanvasPointerEnd(event), { passive: false });
    this.canvas.addEventListener("pointercancel", (event) => this.onCanvasPointerEnd(event), { passive: false });
    this.canvas.addEventListener("lostpointercapture", (event) => this.onCanvasPointerEnd(event), { passive: false });
  }

  private bindButtons(): void {
    for (const button of this.buttons) {
      button.addEventListener("pointerdown", (event) => this.onButtonPointerDown(event), { passive: false });
      button.addEventListener("pointerup", (event) => this.onButtonPointerEnd(event), { passive: false });
      button.addEventListener("pointercancel", (event) => this.onButtonPointerEnd(event), { passive: false });
      button.addEventListener("lostpointercapture", (event) => this.onButtonPointerEnd(event), { passive: false });
    }
  }

  private onCanvasPointerDown(event: PointerEvent): void {
    if (!isTouchPointer(event)) {
      return;
    }
    this.markTouchEvent();
    event.preventDefault();
    this.setVisible(true);
    this.canvas.focus();
    const rect = this.canvas.getBoundingClientRect();
    const onLeft = event.clientX < rect.left + rect.width * 0.5;
    if (onLeft && this.movementPointerId === null) {
      this.startMovement(event);
    } else if (this.lookPointerId === null) {
      this.startLook(event);
    }
  }

  private onCanvasPointerMove(event: PointerEvent): void {
    if (event.pointerId === this.movementPointerId) {
      this.markTouchEvent();
      event.preventDefault();
      this.updateMovement(event.clientX, event.clientY);
    } else if (event.pointerId === this.lookPointerId) {
      this.markTouchEvent();
      event.preventDefault();
      this.updateLook(event.clientX, event.clientY);
    }
  }

  private onCanvasPointerEnd(event: PointerEvent): void {
    if (event.pointerId === this.movementPointerId) {
      this.markTouchEvent();
      event.preventDefault();
      this.clearMovement();
    } else if (event.pointerId === this.lookPointerId) {
      this.markTouchEvent();
      event.preventDefault();
      this.clearLook();
    }
  }

  private onButtonPointerDown(event: PointerEvent): void {
    // currentTarget is the bound `[data-touch-key]` button (an HTMLElement) for the duration of
    // its own dispatch.
    const target = event.currentTarget as HTMLElement | null;
    const key = target?.dataset?.touchKey;
    if (!key) {
      return;
    }
    this.markTouchEvent();
    event.preventDefault();
    event.stopPropagation();
    this.setVisible(true);
    trySetPointerCapture(target, event.pointerId);
    this.buttonPointers.set(event.pointerId, key);
    target.dataset.active = "true";
    this.app.setTouchKey(key, true);
    this.updateRuntimeState();
  }

  private onButtonPointerEnd(event: PointerEvent): void {
    const key = this.buttonPointers.get(event.pointerId);
    if (!key) {
      return;
    }
    this.markTouchEvent();
    event.preventDefault();
    event.stopPropagation();
    this.buttonPointers.delete(event.pointerId);
    const button = this.buttons.find((candidate) => candidate.dataset.touchKey === key);
    if (button) {
      button.dataset.active = "false";
    }
    this.app.setTouchKey(key, false);
    this.updateRuntimeState();
  }

  private startMovement(event: PointerEvent): void {
    this.movementPointerId = event.pointerId;
    this.movementBaseX = event.clientX;
    this.movementBaseY = event.clientY;
    this.movementThumbX = event.clientX;
    this.movementThumbY = event.clientY;
    trySetPointerCapture(this.canvas, event.pointerId);
    this.updateJoystickVisual();
    this.updateMovement(event.clientX, event.clientY);
    this.updateRuntimeState();
  }

  private updateMovement(clientX: number, clientY: number): void {
    const dx = clientX - this.movementBaseX;
    const dy = clientY - this.movementBaseY;
    const distance = Math.hypot(dx, dy);
    const scale = distance > TOUCH_JOYSTICK_MAX_DISTANCE
      ? TOUCH_JOYSTICK_MAX_DISTANCE / distance
      : 1;
    const clampedX = dx * scale;
    const clampedY = dy * scale;
    this.movementThumbX = this.movementBaseX + clampedX;
    this.movementThumbY = this.movementBaseY + clampedY;
    const axisX = clampedX / TOUCH_JOYSTICK_MAX_DISTANCE;
    const axisY = -clampedY / TOUCH_JOYSTICK_MAX_DISTANCE;
    const magnitude = Math.hypot(axisX, axisY);
    const active = magnitude >= TOUCH_AXIS_THRESHOLD;
    let leftImpulse = 0;
    let forwardImpulse = 0;
    if (active) {
      const adjustedMagnitude = (magnitude - TOUCH_AXIS_THRESHOLD) / (1 - TOUCH_AXIS_THRESHOLD);
      const impulseScale = adjustedMagnitude / magnitude;
      leftImpulse = -axisX * impulseScale;
      forwardImpulse = axisY * impulseScale;
    }
    this.app.setTouchMovementImpulse(leftImpulse, forwardImpulse, true);
    this.app.setTouchKeys({
      forward: active && axisY > TOUCH_AXIS_THRESHOLD,
      backward: active && axisY < -TOUCH_AXIS_THRESHOLD,
      left: active && axisX < -TOUCH_AXIS_THRESHOLD,
      right: active && axisX > TOUCH_AXIS_THRESHOLD,
    });
    this.updateJoystickVisual();
    this.updateRuntimeState();
  }

  private clearMovement(): void {
    this.movementPointerId = null;
    this.app.setTouchMovementImpulse(0, 0, false);
    this.app.setTouchKeys({
      forward: false,
      backward: false,
      left: false,
      right: false,
    });
    if (this.joystick) {
      this.joystick.dataset.active = "false";
    }
    this.updateRuntimeState();
  }

  private startLook(event: PointerEvent): void {
    this.lookPointerId = event.pointerId;
    this.lookLastX = event.clientX;
    this.lookLastY = event.clientY;
    trySetPointerCapture(this.canvas, event.pointerId);
    this.updateRuntimeState();
  }

  private updateLook(clientX: number, clientY: number): void {
    const sensitivity = Number.isFinite(this.app.lookSensitivity) ? this.app.lookSensitivity : 1;
    this.app.queueMouseDelta(
      (clientX - this.lookLastX) * sensitivity,
      (clientY - this.lookLastY) * sensitivity,
    );
    this.lookLastX = clientX;
    this.lookLastY = clientY;
    this.updateRuntimeState();
  }

  private clearLook(): void {
    this.lookPointerId = null;
    this.updateRuntimeState();
  }

  clearAll(): void {
    this.clearMovement();
    this.clearLook();
    for (const pointerId of Array.from(this.buttonPointers.keys())) {
      const key = this.buttonPointers.get(pointerId);
      if (key) {
        this.app.setTouchKey(key, false);
      }
      this.buttonPointers.delete(pointerId);
    }
    for (const button of this.buttons) {
      button.dataset.active = "false";
    }
    this.updateRuntimeState();
  }

  private updateJoystickVisual(): void {
    if (!this.joystick || !this.thumb) {
      return;
    }
    const rect = this.canvas.getBoundingClientRect();
    this.joystick.style.left = `${this.movementBaseX - rect.left}px`;
    this.joystick.style.top = `${this.movementBaseY - rect.top}px`;
    this.joystick.dataset.active = this.movementPointerId === null ? "false" : "true";
    this.thumb.style.left = `${50 + (this.movementThumbX - this.movementBaseX)}px`;
    this.thumb.style.top = `${50 + (this.movementThumbY - this.movementBaseY)}px`;
  }

  setVisible(visible: boolean): void {
    if (this.root) {
      this.root.dataset.visible = visible ? "true" : "false";
      this.root.setAttribute("aria-hidden", visible ? "false" : "true");
    }
    this.runtimeState.touchControlsVisible = Boolean(visible);
  }

  private markTouchEvent(): void {
    this.lastTouchAt = performance.now();
  }

  shouldIgnoreMouseEvent(): boolean {
    return performance.now() - this.lastTouchAt < 800;
  }

  private updateRuntimeState(): void {
    this.runtimeState.touchControlsVisible = this.root?.dataset.visible === "true";
    this.runtimeState.touchJoystickActive = this.movementPointerId !== null;
    this.runtimeState.touchMovementLeftImpulse = this.app.touchMovementImpulse.left;
    this.runtimeState.touchMovementForwardImpulse = this.app.touchMovementImpulse.forward;
    this.runtimeState.touchLookActive = this.lookPointerId !== null;
    this.runtimeState.touchButtonActiveCount = this.buttonPointers.size;
  }

  snapshot(): TouchControlSnapshot {
    return {
      visible: this.root?.dataset.visible === "true",
      joystickActive: this.movementPointerId !== null,
      lookActive: this.lookPointerId !== null,
      buttonActiveCount: this.buttonPointers.size,
      keys: { ...this.app.touchKeys },
      movementImpulse: this.app.currentMovementImpulse(),
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
