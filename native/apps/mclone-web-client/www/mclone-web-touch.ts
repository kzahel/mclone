// Native web touch-control glue. Deploy note: mclone-web-app.js is
// cache-busted with `?v=<version>`, but a static `import` of this module cannot
// carry that query string, so a deploy that bumps the asset version must rely
// on HTTP cache revalidation of this bare URL (smoke/dev leave the version
// unset).

const TOUCH_JOYSTICK_MAX_DISTANCE = 50;
const TOUCH_JOYSTICK_DEAD_ZONE = 10;
const TOUCH_AXIS_THRESHOLD = TOUCH_JOYSTICK_DEAD_ZONE / TOUCH_JOYSTICK_MAX_DISTANCE;
const TOUCH_MENU_LEFT = 10;
const TOUCH_MENU_TOP = 10;
const TOUCH_MENU_SIZE = 40;
const TOUCH_BUTTON_SIZE = 58;
const TOUCH_BUTTON_GAP = 12;
const TOUCH_BUTTON_RIGHT = 18;
const TOUCH_BUTTON_BOTTOM = 24;

export interface TouchMovementImpulse {
  active: boolean;
  left: number;
  forward: number;
}

export interface TouchOverlayState {
  visible: boolean;
  movementActive: boolean;
  movementBaseX: number;
  movementBaseY: number;
  movementThumbX: number;
  movementThumbY: number;
  jumpPressed: boolean;
  sprintPressed: boolean;
  descendPressed: boolean;
  menuPressed: boolean;
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
  handleNativeUiPointerMove(clientX: number, clientY: number, pointerType?: string): Record<string, any> | null;
  handleNativeUiPointerDown(clientX: number, clientY: number, pointerType?: string): Record<string, any> | null;
  handleNativeUiPointerUp(clientX: number, clientY: number, pointerType?: string): Record<string, any> | null;
  setNativeTouchLookSensitivity(value: number, available?: boolean, persist?: boolean): Record<string, any> | null;
  setNativeTouchControlsOverlay(overlay: TouchOverlayState): Record<string, any> | null;
  openNativePauseUi(): Record<string, any> | null;
  closeNativeUi(): Record<string, any> | null;
}

interface TouchRuntimeState extends Record<string, any> {
  touchControlsVisible?: boolean;
  touchJoystickActive?: boolean;
  touchMovementLeftImpulse?: number;
  touchMovementForwardImpulse?: number;
  touchLookActive?: boolean;
  touchButtonActiveCount?: number;
  uiActive?: boolean;
}

export interface TouchControlSnapshot {
  visible: boolean;
  joystickActive: boolean;
  lookActive: boolean;
  buttonActiveCount: number;
  keys: Record<string, boolean>;
  movementImpulse: TouchMovementImpulse;
}

type TouchButtonKey = "jump" | "sprint" | "descend";

export class TouchControls {
  private readonly app: TouchControlApp;
  private readonly runtimeState: TouchRuntimeState;
  readonly canvas: HTMLCanvasElement;
  private visible: boolean;
  private movementPointerId: number | null;
  private lookPointerId: number | null;
  private menuPointerId: number | null;
  private movementBaseX: number;
  private movementBaseY: number;
  private movementThumbX: number;
  private movementThumbY: number;
  private lookLastX: number;
  private lookLastY: number;
  private readonly buttonPointers: Map<number, TouchButtonKey>;
  private lastTouchAt: number;

  constructor(app: TouchControlApp, runtimeState: TouchRuntimeState) {
    this.app = app;
    this.runtimeState = runtimeState;
    this.canvas = app.canvas;
    this.visible = false;
    this.movementPointerId = null;
    this.lookPointerId = null;
    this.menuPointerId = null;
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
    window.addEventListener("blur", () => this.clearAll());
  }

  private bindCanvas(): void {
    this.canvas.addEventListener("pointerdown", (event) => this.onCanvasPointerDown(event), { passive: false });
    this.canvas.addEventListener("pointermove", (event) => this.onCanvasPointerMove(event), { passive: false });
    this.canvas.addEventListener("pointerup", (event) => this.onCanvasPointerEnd(event), { passive: false });
    this.canvas.addEventListener("pointercancel", (event) => this.onCanvasPointerEnd(event), { passive: false });
    this.canvas.addEventListener("lostpointercapture", (event) => this.onCanvasPointerEnd(event), { passive: false });
  }

  private onCanvasPointerDown(event: PointerEvent): void {
    if (!isTouchPointer(event)) {
      return;
    }
    this.markTouchEvent();
    event.preventDefault();
    this.setVisible(true);
    this.canvas.focus();

    const point = this.canvasPoint(event.clientX, event.clientY);
    if (touchMenuRect().contains(point)) {
      this.startMenu(event);
      return;
    }
    if (this.runtimeState.uiActive === true) {
      this.app.handleNativeUiPointerDown(event.clientX, event.clientY, "touch");
      return;
    }
    const buttonKey = this.touchButtonAt(point);
    if (buttonKey) {
      this.startButton(event, buttonKey);
      return;
    }

    const rect = this.canvas.getBoundingClientRect();
    const onLeft = event.clientX < rect.left + rect.width * 0.5;
    if (onLeft && this.movementPointerId === null) {
      this.startMovement(event);
    } else if (this.lookPointerId === null) {
      this.startLook(event);
    }
  }

  private onCanvasPointerMove(event: PointerEvent): void {
    if (!isTouchPointer(event)) {
      return;
    }
    if (this.runtimeState.uiActive === true && !this.hasCapturedOverlayPointer(event.pointerId)) {
      this.markTouchEvent();
      event.preventDefault();
      this.app.handleNativeUiPointerMove(event.clientX, event.clientY, "touch");
      return;
    }
    if (event.pointerId === this.movementPointerId) {
      this.markTouchEvent();
      event.preventDefault();
      this.updateMovement(event.clientX, event.clientY);
    } else if (event.pointerId === this.lookPointerId) {
      this.markTouchEvent();
      event.preventDefault();
      this.updateLook(event.clientX, event.clientY);
    } else if (this.hasCapturedOverlayPointer(event.pointerId)) {
      this.markTouchEvent();
      event.preventDefault();
    }
  }

  private onCanvasPointerEnd(event: PointerEvent): void {
    if (event.pointerId === this.menuPointerId) {
      this.markTouchEvent();
      event.preventDefault();
      this.clearMenu();
      return;
    }
    if (this.buttonPointers.has(event.pointerId)) {
      this.markTouchEvent();
      event.preventDefault();
      this.clearButton(event.pointerId);
      return;
    }
    if (this.runtimeState.uiActive === true && isTouchPointer(event)) {
      this.markTouchEvent();
      event.preventDefault();
      this.app.handleNativeUiPointerUp(event.clientX, event.clientY, "touch");
      return;
    }
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

  private hasCapturedOverlayPointer(pointerId: number): boolean {
    return pointerId === this.menuPointerId || this.buttonPointers.has(pointerId);
  }

  private startMovement(event: PointerEvent): void {
    const point = this.canvasPoint(event.clientX, event.clientY);
    this.movementPointerId = event.pointerId;
    this.movementBaseX = point.x;
    this.movementBaseY = point.y;
    this.movementThumbX = point.x;
    this.movementThumbY = point.y;
    trySetPointerCapture(this.canvas, event.pointerId);
    this.updateMovement(event.clientX, event.clientY);
    this.updateRuntimeState();
  }

  private updateMovement(clientX: number, clientY: number): void {
    const point = this.canvasPoint(clientX, clientY);
    const dx = point.x - this.movementBaseX;
    const dy = point.y - this.movementBaseY;
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

  private startButton(event: PointerEvent, key: TouchButtonKey): void {
    trySetPointerCapture(this.canvas, event.pointerId);
    this.buttonPointers.set(event.pointerId, key);
    this.app.setTouchKey(key, true);
    this.updateRuntimeState();
  }

  private clearButton(pointerId: number): void {
    const key = this.buttonPointers.get(pointerId);
    if (!key) {
      return;
    }
    this.buttonPointers.delete(pointerId);
    this.app.setTouchKey(key, false);
    this.updateRuntimeState();
  }

  private startMenu(event: PointerEvent): void {
    this.menuPointerId = event.pointerId;
    trySetPointerCapture(this.canvas, event.pointerId);
    if (this.runtimeState.uiActive === true) {
      this.app.closeNativeUi();
    } else {
      this.app.openNativePauseUi();
    }
    this.updateRuntimeState();
  }

  private clearMenu(): void {
    this.menuPointerId = null;
    this.updateRuntimeState();
  }

  clearAll(): void {
    this.clearMovement();
    this.clearLook();
    for (const pointerId of Array.from(this.buttonPointers.keys())) {
      this.clearButton(pointerId);
    }
    this.menuPointerId = null;
    this.updateRuntimeState();
  }

  setVisible(visible: boolean): void {
    this.visible = Boolean(visible);
    this.runtimeState.touchControlsVisible = this.visible;
    this.runtimeState.touchLookSensitivityAvailable = this.visible;
    this.app.setNativeTouchLookSensitivity(this.app.lookSensitivity, this.visible, false);
    this.updateRuntimeState();
  }

  private markTouchEvent(): void {
    this.lastTouchAt = performance.now();
  }

  shouldIgnoreMouseEvent(): boolean {
    return performance.now() - this.lastTouchAt < 800;
  }

  private updateRuntimeState(): void {
    this.runtimeState.touchControlsVisible = this.visible;
    this.runtimeState.touchJoystickActive = this.movementPointerId !== null;
    this.runtimeState.touchMovementLeftImpulse = this.app.touchMovementImpulse.left;
    this.runtimeState.touchMovementForwardImpulse = this.app.touchMovementImpulse.forward;
    this.runtimeState.touchLookActive = this.lookPointerId !== null;
    this.runtimeState.touchButtonActiveCount = this.buttonPointers.size;
    this.app.setNativeTouchControlsOverlay(this.overlayState());
  }

  snapshot(): TouchControlSnapshot {
    return {
      visible: this.visible,
      joystickActive: this.movementPointerId !== null,
      lookActive: this.lookPointerId !== null,
      buttonActiveCount: this.buttonPointers.size,
      keys: { ...this.app.touchKeys },
      movementImpulse: this.app.currentMovementImpulse(),
    };
  }

  private overlayState(): TouchOverlayState {
    return {
      visible: this.visible,
      movementActive: this.movementPointerId !== null,
      movementBaseX: this.movementBaseX,
      movementBaseY: this.movementBaseY,
      movementThumbX: this.movementThumbX,
      movementThumbY: this.movementThumbY,
      jumpPressed: this.isButtonPressed("jump"),
      sprintPressed: this.isButtonPressed("sprint"),
      descendPressed: this.isButtonPressed("descend"),
      menuPressed: this.menuPointerId !== null,
    };
  }

  private isButtonPressed(key: TouchButtonKey): boolean {
    for (const pressed of this.buttonPointers.values()) {
      if (pressed === key) {
        return true;
      }
    }
    return false;
  }

  private canvasPoint(clientX: number, clientY: number): { x: number, y: number } {
    const rect = this.canvas.getBoundingClientRect();
    return {
      x: clientX - rect.left,
      y: clientY - rect.top,
    };
  }

  private touchButtonAt(point: { x: number, y: number }): TouchButtonKey | null {
    const rects = touchButtonRects(this.canvas);
    if (rects.jump.contains(point)) {
      return "jump";
    }
    if (rects.sprint.contains(point)) {
      return "sprint";
    }
    if (rects.descend.contains(point)) {
      return "descend";
    }
    return null;
  }
}

export function hasTouchInput(): boolean {
  return (typeof navigator !== "undefined" && navigator.maxTouchPoints > 0)
    || window.matchMedia("(pointer: coarse)").matches;
}

interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
  contains(point: { x: number, y: number }): boolean;
}

function rect(x: number, y: number, width: number, height: number): Rect {
  return {
    x,
    y,
    width,
    height,
    contains(point: { x: number, y: number }): boolean {
      return point.x >= x
        && point.y >= y
        && point.x < x + width
        && point.y < y + height;
    },
  };
}

function touchMenuRect(): Rect {
  return rect(TOUCH_MENU_LEFT, TOUCH_MENU_TOP, TOUCH_MENU_SIZE, TOUCH_MENU_SIZE);
}

function touchButtonRects(canvas: HTMLCanvasElement): { jump: Rect, sprint: Rect, descend: Rect } {
  const bounds = canvas.getBoundingClientRect();
  const x1 = Math.max(0, bounds.width - TOUCH_BUTTON_RIGHT - TOUCH_BUTTON_SIZE);
  const x0 = Math.max(0, x1 - TOUCH_BUTTON_GAP - TOUCH_BUTTON_SIZE);
  const y1 = Math.max(0, bounds.height - TOUCH_BUTTON_BOTTOM - TOUCH_BUTTON_SIZE);
  const y0 = Math.max(0, y1 - TOUCH_BUTTON_GAP - TOUCH_BUTTON_SIZE);
  return {
    jump: rect(x1, y0, TOUCH_BUTTON_SIZE, TOUCH_BUTTON_SIZE),
    sprint: rect(x0, y1, TOUCH_BUTTON_SIZE, TOUCH_BUTTON_SIZE),
    descend: rect(x1, y1, TOUCH_BUTTON_SIZE, TOUCH_BUTTON_SIZE),
  };
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
