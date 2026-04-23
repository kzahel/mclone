// Debug tooling — see tactical 40. Throwaway when real Player/Input lands.

export interface DebugInputFrame {
  readonly heldKeys: ReadonlySet<string>;
  readonly mouseDeltaX: number;
  readonly mouseDeltaY: number;
  readonly locked: boolean;
}

export class DebugInput {
  private readonly heldKeys = new Set<string>();
  private mouseDeltaX = 0;
  private mouseDeltaY = 0;
  private locked = false;

  public constructor(canvas: HTMLCanvasElement) {
    canvas.addEventListener("click", () => {
      if (!this.locked) {
        canvas.requestPointerLock();
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
  }

  public consumeFrame(): DebugInputFrame {
    const frame: DebugInputFrame = {
      heldKeys: new Set(this.heldKeys),
      mouseDeltaX: this.mouseDeltaX,
      mouseDeltaY: this.mouseDeltaY,
      locked: this.locked,
    };
    this.mouseDeltaX = 0;
    this.mouseDeltaY = 0;
    return frame;
  }
}
