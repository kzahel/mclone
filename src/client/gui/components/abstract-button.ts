import type { Font } from "../font";
import { AbstractWidget } from "./abstract-widget";

export abstract class AbstractButton extends AbstractWidget {
  public constructor(x: number, y: number, width: number, height: number, message: string, font: Font) {
    super(x, y, width, height, message, font);
  }

  public abstract onPress(): void;

  public override onClick(_mouseX: number, _mouseY: number): void {
    this.onPress();
  }

  public keyPressed(keyCode: number, _scanCode: number, _modifiers: number): boolean {
    if (!this.active || !this.visible) {
      return false;
    }
    if (keyCode !== 257 && keyCode !== 32 && keyCode !== 335) {
      return false;
    }

    this.playDownSound();
    this.onPress();
    return true;
  }
}
