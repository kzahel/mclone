import type { ScreenManager } from "../../client/gui/screen-manager";
import { GuiDrawList } from "./gui-draw-list";
import { GuiRenderer } from "./gui-renderer";

export interface GuiInputAttachment {
  dispose(): void;
}

export type GuiOverlayRenderer = (drawList: GuiDrawList, mouseX: number, mouseY: number, partialTick: number) => void;
const NOOP_GUI_OVERLAY_RENDERER: GuiOverlayRenderer = () => {};

export class GuiOverlayHost {
  private readonly drawList = new GuiDrawList();
  private mouseX = -1;
  private mouseY = -1;

  private constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly screenManager: ScreenManager,
    private readonly renderer: GuiRenderer,
    private renderOverlay: GuiOverlayRenderer = NOOP_GUI_OVERLAY_RENDERER,
  ) {}

  public static async create(
    device: GPUDevice,
    canvas: HTMLCanvasElement,
    screenManager: ScreenManager,
    renderOverlay?: GuiOverlayRenderer,
  ): Promise<GuiOverlayHost> {
    return new GuiOverlayHost(canvas, screenManager, await GuiRenderer.create(device), renderOverlay);
  }

  public attachInput(onInput?: () => void): GuiInputAttachment {
    const listeners: Array<readonly [EventTarget, string, EventListener]> = [];
    const add = (target: EventTarget, type: string, listener: EventListener): void => {
      target.addEventListener(type, listener);
      listeners.push([target, type, listener]);
    };

    add(this.canvas, "pointermove", (event) => {
      const pointerEvent = event as PointerEvent;
      const previousMouseX = this.mouseX;
      const previousMouseY = this.mouseY;
      this.updateMouseFromClient(pointerEvent.clientX, pointerEvent.clientY);
      if ((pointerEvent.buttons & 1) !== 0) {
        this.screenManager.mouseDragged(this.mouseX, this.mouseY, 0, this.mouseX - previousMouseX, this.mouseY - previousMouseY);
      } else {
        this.screenManager.mouseMoved(this.mouseX, this.mouseY);
      }
      onInput?.();
    });
    add(this.canvas, "pointerdown", (event) => {
      const pointerEvent = event as PointerEvent;
      this.updateMouseFromClient(pointerEvent.clientX, pointerEvent.clientY);
      this.capturePointer(pointerEvent.pointerId);
      if (this.screenManager.mouseClicked(this.mouseX, this.mouseY, pointerEvent.button)) {
        pointerEvent.preventDefault();
      }
      onInput?.();
    });
    add(this.canvas, "pointerup", (event) => {
      const pointerEvent = event as PointerEvent;
      this.updateMouseFromClient(pointerEvent.clientX, pointerEvent.clientY);
      if (this.canvas.hasPointerCapture(pointerEvent.pointerId)) {
        this.canvas.releasePointerCapture(pointerEvent.pointerId);
      }
      if (this.screenManager.mouseReleased(this.mouseX, this.mouseY, pointerEvent.button)) {
        pointerEvent.preventDefault();
      }
      onInput?.();
    });
    add(this.canvas, "wheel", (event) => {
      const wheelEvent = event as WheelEvent;
      this.updateMouseFromClient(wheelEvent.clientX, wheelEvent.clientY);
      if (this.screenManager.mouseScrolled(this.mouseX, this.mouseY, wheelEvent.deltaY)) {
        wheelEvent.preventDefault();
      }
      onInput?.();
    });
    add(window, "keydown", (event) => {
      const keyboardEvent = event as KeyboardEvent;
      if (this.screenManager.keyPressed(keyCodeFromKeyboardEvent(keyboardEvent), 0, keyboardModifiers(keyboardEvent))) {
        keyboardEvent.preventDefault();
      }
      onInput?.();
    });
    add(window, "keyup", (event) => {
      const keyboardEvent = event as KeyboardEvent;
      if (this.screenManager.keyReleased(keyCodeFromKeyboardEvent(keyboardEvent), 0, keyboardModifiers(keyboardEvent))) {
        keyboardEvent.preventDefault();
      }
      onInput?.();
    });

    return {
      dispose: () => {
        for (const [target, type, listener] of listeners) {
          target.removeEventListener(type, listener);
        }
      },
    };
  }

  public setOverlayRenderer(renderOverlay?: GuiOverlayRenderer): void {
    this.renderOverlay = renderOverlay ?? NOOP_GUI_OVERLAY_RENDERER;
  }

  public encode(
    encoder: GPUCommandEncoder,
    view: GPUTextureView,
    format: GPUTextureFormat,
    pixelWidth: number,
    pixelHeight: number,
  ): void {
    const size = GuiRenderer.calculateGuiSize(pixelWidth, pixelHeight);
    this.screenManager.resize(size.guiWidth, size.guiHeight);
    this.screenManager.tick();
    this.drawList.clear();
    this.screenManager.render(this.drawList, this.mouseX, this.mouseY, 0);
    this.renderOverlay(this.drawList, this.mouseX, this.mouseY, 0);
    this.renderer.render(
      this.drawList,
      {
        view,
        format,
        pixelWidth,
        pixelHeight,
        guiWidth: size.guiWidth,
        guiHeight: size.guiHeight,
      },
      encoder,
    );
  }

  public destroy(): void {
    this.renderer.destroy();
  }

  private updateMouseFromClient(clientX: number, clientY: number): void {
    const rect = this.canvas.getBoundingClientRect();
    const size = GuiRenderer.calculateGuiSize(this.canvas.width, this.canvas.height);
    const cssX = rect.width <= 0 ? 0 : clientX - rect.left;
    const cssY = rect.height <= 0 ? 0 : clientY - rect.top;
    this.mouseX = Math.floor((cssX / Math.max(1, rect.width)) * size.guiWidth);
    this.mouseY = Math.floor((cssY / Math.max(1, rect.height)) * size.guiHeight);
  }

  private capturePointer(pointerId: number): void {
    if (this.canvas.hasPointerCapture(pointerId)) {
      return;
    }

    try {
      this.canvas.setPointerCapture(pointerId);
    } catch (error) {
      if (!(error instanceof DOMException) || (error.name !== "InvalidStateError" && error.name !== "NotFoundError")) {
        throw error;
      }
    }
  }
}

function keyCodeFromKeyboardEvent(event: KeyboardEvent): number {
  switch (event.key) {
    case "Escape":
      return 256;
    case "Enter":
      return 257;
    case "Tab":
      return 258;
    case "ArrowLeft":
      return 263;
    case "ArrowRight":
      return 262;
    case " ":
      return 32;
    default:
      return event.key.length === 1 ? event.key.toUpperCase().charCodeAt(0) : 0;
  }
}

function keyboardModifiers(event: KeyboardEvent): number {
  return (event.shiftKey ? 1 : 0)
    | (event.ctrlKey ? 2 : 0)
    | (event.altKey ? 4 : 0)
    | (event.metaKey ? 8 : 0);
}
