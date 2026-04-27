import { hitTouchMoveButton, type TouchMoveButton, type TouchControlTarget } from "./touch-control-layout";

// Debug tooling — see tactical 40. Throwaway when real Player/Input lands.

export interface DebugInputFrame {
  readonly heldKeys: ReadonlySet<string>;
  readonly mouseDeltaX: number;
  readonly mouseDeltaY: number;
  readonly locked: boolean;
  readonly joystickX: number;
  readonly joystickY: number;
  readonly moveForward: boolean;
  readonly moveBack: boolean;
  readonly flyUp: boolean;
  readonly flyDown: boolean;
}

export interface DebugInputOptions {
  readonly isGuiActive?: () => boolean;
  readonly requirePointerLock?: boolean;
  readonly onPointerLockChange?: (locked: boolean) => void;
}

export interface TouchJoystickState {
  readonly baseX: number;
  readonly baseY: number;
  readonly thumbX: number;
  readonly thumbY: number;
  readonly maxDistance: number;
}

export interface TouchControlsState {
  readonly joystick: TouchJoystickState | null;
  readonly moveForward: boolean;
  readonly moveBack: boolean;
  readonly visible: boolean;
}

// Port of tilefun/src/input/TouchJoystick.ts — fixed-center virtual joystick.
const JOYSTICK_MAX_DISTANCE = 50;
const JOYSTICK_DEAD_ZONE = 10;

interface JoystickTouch {
  readonly id: number;
  readonly baseX: number;
  readonly baseY: number;
  thumbX: number;
  thumbY: number;
}

interface LookTouch {
  readonly id: number;
  lastX: number;
  lastY: number;
}

export class DebugInput {
  private readonly heldKeys = new Set<string>();
  private readonly listeners: Array<readonly [EventTarget, string, EventListener]> = [];
  private readonly moveButtonTouches = new Map<number, TouchMoveButton>();
  private touchControlsVisible = hasTouchInput();
  private mouseDeltaX = 0;
  private mouseDeltaY = 0;
  private locked = false;
  private eventCount = 0;

  private joystickTouch: JoystickTouch | null = null;
  private lookTouch: LookTouch | null = null;
  private moveForward = false;
  private moveBack = false;
  private flyUp = false;
  private flyDown = false;

  public constructor(canvas: HTMLCanvasElement, private readonly options: DebugInputOptions = {}) {
    this.add(canvas, "click", () => {
      if (this.isGuiActive()) {
        return;
      }
      this.eventCount++;
      if (this.requiresPointerLock() && !this.locked) {
        canvas.requestPointerLock().catch(() => {
          // Mobile / no pointer-lock — touch handlers take over.
        });
      }
    });
    this.add(document, "pointerlockchange", () => {
      this.locked = document.pointerLockElement === canvas;
      this.eventCount++;
      if (!this.locked) {
        this.heldKeys.clear();
        this.mouseDeltaX = 0;
        this.mouseDeltaY = 0;
      }
      this.options.onPointerLockChange?.(this.locked);
    });
    this.add(document, "keydown", (event) => {
      const keyboardEvent = event as KeyboardEvent;
      if (event.defaultPrevented) {
        return;
      }
      if (this.isGuiActive()) {
        this.clearGameplayInput();
        this.eventCount++;
        return;
      }
      if (this.requiresPointerLock() && !this.locked) return;
      this.heldKeys.add(keyboardEvent.code);
      this.eventCount++;
      if (keyboardEvent.code === "Space" || keyboardEvent.code.startsWith("Arrow")) {
        event.preventDefault();
      }
    });
    this.add(document, "keyup", (event) => {
      const keyboardEvent = event as KeyboardEvent;
      if (event.defaultPrevented) {
        return;
      }
      this.heldKeys.delete(keyboardEvent.code);
      this.eventCount++;
    });
    this.add(document, "mousemove", (event) => {
      const mouseEvent = event as MouseEvent;
      if (event.defaultPrevented) {
        return;
      }
      if (this.isGuiActive()) {
        this.mouseDeltaX = 0;
        this.mouseDeltaY = 0;
        this.eventCount++;
        return;
      }
      if (this.requiresPointerLock() && !this.locked) return;
      this.mouseDeltaX += mouseEvent.movementX;
      this.mouseDeltaY += mouseEvent.movementY;
      this.eventCount++;
    });

    this.add(canvas, "touchstart", (event) => this.onTouchStart(event as TouchEvent, canvas), { passive: false });
    this.add(canvas, "touchmove", (event) => this.onTouchMove(event as TouchEvent), { passive: false });
    this.add(canvas, "touchend", (event) => this.onTouchEnd(event as TouchEvent), { passive: false });
    this.add(canvas, "touchcancel", (event) => this.onTouchEnd(event as TouchEvent), { passive: false });
  }

  public consumeFrame(): DebugInputFrame {
    if (this.isGuiActive()) {
      this.clearGameplayInput();
    }
    const [jx, jy] = this.readJoystickAxes();
    const frame: DebugInputFrame = {
      heldKeys: new Set(this.heldKeys),
      mouseDeltaX: this.mouseDeltaX,
      mouseDeltaY: this.mouseDeltaY,
      locked:
        this.locked
        || this.joystickTouch !== null
        || this.lookTouch !== null
        || this.moveForward
        || this.moveBack
        || this.flyUp
        || this.flyDown,
      joystickX: jx,
      joystickY: jy,
      moveForward: this.moveForward,
      moveBack: this.moveBack,
      flyUp: this.flyUp,
      flyDown: this.flyDown,
    };
    this.mouseDeltaX = 0;
    this.mouseDeltaY = 0;
    return frame;
  }

  public getEventCount(): number {
    return this.eventCount;
  }

  public getTouchJoystickState(): TouchJoystickState | null {
    if (this.joystickTouch === null) {
      return null;
    }

    return {
      baseX: this.joystickTouch.baseX,
      baseY: this.joystickTouch.baseY,
      thumbX: this.joystickTouch.thumbX,
      thumbY: this.joystickTouch.thumbY,
      maxDistance: JOYSTICK_MAX_DISTANCE,
    };
  }

  public getTouchControlsState(): TouchControlsState {
    return {
      joystick: this.getTouchJoystickState(),
      moveForward: this.moveForward,
      moveBack: this.moveBack,
      visible: this.touchControlsVisible,
    };
  }

  public dispose(): void {
    for (const [target, type, listener] of this.listeners) {
      target.removeEventListener(type, listener);
    }
    this.listeners.length = 0;
    this.clearGameplayInput();
  }

  private readJoystickAxes(): [number, number] {
    if (!this.joystickTouch) return [0, 0];
    const dx = (this.joystickTouch.thumbX - this.joystickTouch.baseX) / JOYSTICK_MAX_DISTANCE;
    const dy = (this.joystickTouch.thumbY - this.joystickTouch.baseY) / JOYSTICK_MAX_DISTANCE;
    const magnitude = Math.hypot(dx, dy);
    if (magnitude < JOYSTICK_DEAD_ZONE / JOYSTICK_MAX_DISTANCE) return [0, 0];
    if (magnitude > 1) return [dx / magnitude, -dy / magnitude];
    return [dx, -dy];
  }

  private onTouchStart(event: TouchEvent, canvas: HTMLCanvasElement): void {
    if (this.isGuiActive()) {
      this.clearGameplayInput();
      this.eventCount++;
      return;
    }
    event.preventDefault();
    this.eventCount++;
    this.touchControlsVisible = true;
    const midpoint = canvas.clientWidth / 2;
    const target = touchControlTargetForCanvas(canvas);
    for (const touch of Array.from(event.changedTouches)) {
      const moveButton = hitTouchMoveButton(touch.clientX, touch.clientY, target);
      if (moveButton !== null) {
        this.moveButtonTouches.set(touch.identifier, moveButton);
        this.updateMoveButtonState();
        continue;
      }

      const onLeft = touch.clientX < midpoint;
      if (onLeft && this.joystickTouch === null) {
        this.joystickTouch = {
          id: touch.identifier,
          baseX: touch.clientX,
          baseY: touch.clientY,
          thumbX: touch.clientX,
          thumbY: touch.clientY,
        };
        this.renderJoystick();
      } else if (!onLeft && this.lookTouch === null) {
        this.lookTouch = {
          id: touch.identifier,
          lastX: touch.clientX,
          lastY: touch.clientY,
        };
      }
    }
  }

  private onTouchMove(event: TouchEvent): void {
    if (this.isGuiActive()) {
      this.clearGameplayInput();
      this.eventCount++;
      return;
    }
    event.preventDefault();
    this.eventCount++;
    this.touchControlsVisible = true;
    const target = touchControlTargetForCanvas(event.currentTarget as HTMLCanvasElement);
    for (const touch of Array.from(event.changedTouches)) {
      if (this.moveButtonTouches.has(touch.identifier)) {
        const moveButton = hitTouchMoveButton(touch.clientX, touch.clientY, target);
        if (moveButton === null) {
          this.moveButtonTouches.delete(touch.identifier);
        } else {
          this.moveButtonTouches.set(touch.identifier, moveButton);
        }
        this.updateMoveButtonState();
      } else if (this.joystickTouch && touch.identifier === this.joystickTouch.id) {
        const dx = touch.clientX - this.joystickTouch.baseX;
        const dy = touch.clientY - this.joystickTouch.baseY;
        const distance = Math.hypot(dx, dy);
        if (distance > JOYSTICK_MAX_DISTANCE) {
          this.joystickTouch.thumbX = this.joystickTouch.baseX + (dx / distance) * JOYSTICK_MAX_DISTANCE;
          this.joystickTouch.thumbY = this.joystickTouch.baseY + (dy / distance) * JOYSTICK_MAX_DISTANCE;
        } else {
          this.joystickTouch.thumbX = touch.clientX;
          this.joystickTouch.thumbY = touch.clientY;
        }
        this.renderJoystick();
      } else if (this.lookTouch && touch.identifier === this.lookTouch.id) {
        this.mouseDeltaX += touch.clientX - this.lookTouch.lastX;
        this.mouseDeltaY += touch.clientY - this.lookTouch.lastY;
        this.lookTouch.lastX = touch.clientX;
        this.lookTouch.lastY = touch.clientY;
      }
    }
  }

  private onTouchEnd(event: TouchEvent): void {
    if (this.isGuiActive()) {
      this.clearGameplayInput();
      this.eventCount++;
      return;
    }
    event.preventDefault();
    this.eventCount++;
    for (const touch of Array.from(event.changedTouches)) {
      if (this.moveButtonTouches.delete(touch.identifier)) {
        this.updateMoveButtonState();
      } else if (this.joystickTouch && touch.identifier === this.joystickTouch.id) {
        this.joystickTouch = null;
        this.renderJoystick();
      } else if (this.lookTouch && touch.identifier === this.lookTouch.id) {
        this.lookTouch = null;
      }
    }
  }

  private renderJoystick(): void {
    // WebGPU: the GUI overlay reads joystick state; keep this path free of DOM rendering.
  }

  private clearGameplayInput(): void {
    this.heldKeys.clear();
    this.mouseDeltaX = 0;
    this.mouseDeltaY = 0;
    this.moveForward = false;
    this.moveBack = false;
    this.flyUp = false;
    this.flyDown = false;
    this.moveButtonTouches.clear();
    this.joystickTouch = null;
    this.lookTouch = null;
    this.renderJoystick();
  }

  private isGuiActive(): boolean {
    return this.options.isGuiActive?.() === true;
  }

  private requiresPointerLock(): boolean {
    return this.options.requirePointerLock !== false;
  }

  private add(
    target: EventTarget,
    type: string,
    listener: EventListener,
    options?: AddEventListenerOptions,
  ): void {
    target.addEventListener(type, listener, options);
    this.listeners.push([target, type, listener]);
  }

  private updateMoveButtonState(): void {
    this.moveForward = false;
    this.moveBack = false;
    for (const button of this.moveButtonTouches.values()) {
      if (button === "forward") {
        this.moveForward = true;
      } else {
        this.moveBack = true;
      }
    }
  }
}

function touchControlTargetForCanvas(canvas: HTMLCanvasElement): TouchControlTarget {
  const rect = canvas.getBoundingClientRect();
  return {
    cssLeft: rect.left,
    cssTop: rect.top,
    cssWidth: rect.width,
    cssHeight: rect.height,
  };
}

function hasTouchInput(): boolean {
  return typeof navigator !== "undefined" && navigator.maxTouchPoints > 0;
}
