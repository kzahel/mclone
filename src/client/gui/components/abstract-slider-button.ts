import type { GuiDrawList } from "../../../renderer/gui/gui-draw-list";
import type { Font } from "../font";
import { AbstractWidget } from "./abstract-widget";

export abstract class AbstractSliderButton extends AbstractWidget {
  protected value: number;

  protected constructor(
    x: number,
    y: number,
    width: number,
    height: number,
    message: string,
    font: Font,
    value: number,
  ) {
    super(x, y, width, height, message, font);
    this.value = clamp01(value);
  }

  protected override getYImage(_hovered: boolean): number {
    return 0;
  }

  protected override renderBg(drawList: GuiDrawList, _mouseX: number, _mouseY: number): void {
    // WebGPU: draw-list textured quads replace the vanilla GL texture bind and blit calls.
    const knobX = this.x + Math.floor(this.value * (this.width - 8));
    this.blit(drawList, knobX, this.y, 0, 66, 4, 20);
    this.blit(drawList, knobX + 4, this.y, 196, 66, 4, 20);
  }

  public override onClick(mouseX: number, _mouseY: number): void {
    this.setValueFromMouse(mouseX);
  }

  public override onRelease(_mouseX: number, _mouseY: number): void {}

  protected override onDrag(mouseX: number, _mouseY: number, _dragX: number, _dragY: number): void {
    this.setValueFromMouse(mouseX);
  }

  public keyPressed(keyCode: number, _scanCode: number, _modifiers: number): boolean {
    if (!this.active || !this.visible) {
      return false;
    }
    if (keyCode !== 263 && keyCode !== 262) {
      return false;
    }

    const previous = this.value;
    const step = 1.0 / (this.width - 8);
    this.setValue(this.value + (keyCode === 263 ? -step : step));
    return previous !== this.value;
  }

  protected abstract updateMessage(): void;

  protected abstract applyValue(): void;

  private setValueFromMouse(mouseX: number): void {
    this.setValue((mouseX - (this.x + 4)) / (this.width - 8));
  }

  protected setValue(value: number): void {
    const nextValue = clamp01(value);
    if (nextValue === this.value) {
      return;
    }

    this.value = nextValue;
    this.applyValue();
    this.updateMessage();
  }
}

function clamp01(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }
  return Math.min(1, Math.max(0, value));
}
