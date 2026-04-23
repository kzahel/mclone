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
  private mouseDeltaX = 0;
  private mouseDeltaY = 0;
  private locked = false;

  private joystickTouch: JoystickTouch | null = null;
  private lookTouch: LookTouch | null = null;
  private moveForward = false;
  private moveBack = false;
  private flyUp = false;
  private flyDown = false;

  private readonly joystickBase: HTMLElement | null;
  private readonly joystickKnob: HTMLElement | null;

  public constructor(canvas: HTMLCanvasElement) {
    this.joystickBase = document.getElementById("joystick-base");
    this.joystickKnob = document.getElementById("joystick-knob");

    canvas.addEventListener("click", () => {
      if (!this.locked) {
        canvas.requestPointerLock().catch(() => {
          // Mobile / no pointer-lock — touch handlers take over.
        });
      }
    });
    document.addEventListener("pointerlockchange", () => {
      this.locked = document.pointerLockElement === canvas;
      if (!this.locked) {
        this.heldKeys.clear();
        this.mouseDeltaX = 0;
        this.mouseDeltaY = 0;
      }
    });
    document.addEventListener("keydown", (event) => {
      if (!this.locked) return;
      this.heldKeys.add(event.code);
      if (event.code === "Space" || event.code.startsWith("Arrow")) {
        event.preventDefault();
      }
    });
    document.addEventListener("keyup", (event) => {
      this.heldKeys.delete(event.code);
    });
    document.addEventListener("mousemove", (event) => {
      if (!this.locked) return;
      this.mouseDeltaX += event.movementX;
      this.mouseDeltaY += event.movementY;
    });

    canvas.addEventListener("touchstart", (event) => this.onTouchStart(event, canvas), { passive: false });
    canvas.addEventListener("touchmove", (event) => this.onTouchMove(event), { passive: false });
    canvas.addEventListener("touchend", (event) => this.onTouchEnd(event), { passive: false });
    canvas.addEventListener("touchcancel", (event) => this.onTouchEnd(event), { passive: false });

    this.bindFlyButton("move-fwd", (held) => { this.moveForward = held; });
    this.bindFlyButton("move-back", (held) => { this.moveBack = held; });
    this.bindFlyButton("fly-up", (held) => { this.flyUp = held; });
    this.bindFlyButton("fly-down", (held) => { this.flyDown = held; });
  }

  public consumeFrame(): DebugInputFrame {
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
    event.preventDefault();
    const midpoint = canvas.clientWidth / 2;
    for (const touch of Array.from(event.changedTouches)) {
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
    event.preventDefault();
    for (const touch of Array.from(event.changedTouches)) {
      if (this.joystickTouch && touch.identifier === this.joystickTouch.id) {
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
    event.preventDefault();
    for (const touch of Array.from(event.changedTouches)) {
      if (this.joystickTouch && touch.identifier === this.joystickTouch.id) {
        this.joystickTouch = null;
        this.renderJoystick();
      } else if (this.lookTouch && touch.identifier === this.lookTouch.id) {
        this.lookTouch = null;
      }
    }
  }

  private renderJoystick(): void {
    if (!this.joystickBase || !this.joystickKnob) return;
    if (!this.joystickTouch) {
      this.joystickBase.classList.remove("active");
      this.joystickKnob.classList.remove("active");
      return;
    }
    this.joystickBase.style.left = `${this.joystickTouch.baseX}px`;
    this.joystickBase.style.top = `${this.joystickTouch.baseY}px`;
    this.joystickKnob.style.left = `${this.joystickTouch.thumbX}px`;
    this.joystickKnob.style.top = `${this.joystickTouch.thumbY}px`;
    this.joystickBase.classList.add("active");
    this.joystickKnob.classList.add("active");
  }

  private bindFlyButton(id: string, setHeld: (held: boolean) => void): void {
    const el = document.getElementById(id);
    if (!el) return;
    const press = (event: Event) => { event.preventDefault(); el.classList.add("active"); setHeld(true); };
    const release = (event: Event) => { event.preventDefault(); el.classList.remove("active"); setHeld(false); };
    el.addEventListener("touchstart", press, { passive: false });
    el.addEventListener("touchend", release, { passive: false });
    el.addEventListener("touchcancel", release, { passive: false });
    el.addEventListener("mousedown", press);
    el.addEventListener("mouseup", release);
    el.addEventListener("mouseleave", release);
  }
}
