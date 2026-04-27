import type { GuiDrawList } from "../../../renderer/gui/gui-draw-list";
import type { Font } from "../font";
import { GuiComponent } from "../gui-component";
import { AbstractWidget } from "./abstract-widget";

export type EditBoxResponder = (value: string) => void;

export class EditBox extends AbstractWidget {
  private cursorPosition: number;

  public constructor(
    x: number,
    y: number,
    width: number,
    height: number,
    private readonly label: string,
    font: Font,
    private value: string,
    private readonly maxLength: number,
    private readonly responder: EditBoxResponder = () => {},
  ) {
    super(x, y, width, height, "", font);
    this.cursorPosition = value.length;
  }

  public getValue(): string {
    return this.value;
  }

  public setValue(value: string): void {
    this.value = this.truncate(value);
    this.cursorPosition = Math.min(this.cursorPosition, this.value.length);
    this.responder(this.value);
  }

  public setFocusedForInput(focused: boolean): void {
    this.setFocused(focused);
  }

  public override mouseClicked(mouseX: number, mouseY: number, button: number): boolean {
    if (!this.active || !this.visible || !this.isValidClickButton(button) || !this.clicked(mouseX, mouseY)) {
      return false;
    }

    this.setFocused(true);
    this.cursorPosition = this.value.length;
    return true;
  }

  public override changeFocus(forward: boolean): boolean {
    const focused = super.changeFocus(forward);
    if (focused) {
      this.cursorPosition = this.value.length;
    }
    return focused;
  }

  public keyPressed(keyCode: number, _scanCode: number, _modifiers: number): boolean {
    if (!this.active || !this.visible || !this.isFocused()) {
      return false;
    }

    switch (keyCode) {
      case 259:
        this.deleteBeforeCursor();
        return true;
      case 261:
        this.deleteAtCursor();
        return true;
      case 263:
        this.cursorPosition = Math.max(0, this.cursorPosition - 1);
        return true;
      case 262:
        this.cursorPosition = Math.min(this.value.length, this.cursorPosition + 1);
        return true;
      case 268:
        this.cursorPosition = 0;
        return true;
      case 269:
        this.cursorPosition = this.value.length;
        return true;
      default:
        return false;
    }
  }

  public charTyped(codePoint: string, modifiers: number): boolean {
    if (!this.active || !this.visible || !this.isFocused() || (modifiers & 0b1110) !== 0) {
      return false;
    }
    if (codePoint.length !== 1 || !isPrintableAscii(codePoint)) {
      return false;
    }
    if (this.value.length >= this.maxLength) {
      return true;
    }

    this.value = `${this.value.slice(0, this.cursorPosition)}${codePoint}${this.value.slice(this.cursorPosition)}`;
    this.cursorPosition++;
    this.responder(this.value);
    return true;
  }

  public override renderButton(drawList: GuiDrawList, mouseX: number, mouseY: number, _partialTick: number): void {
    this.isHovered = mouseX >= this.x && mouseY >= this.y && mouseX < this.x + this.width && mouseY < this.y + this.height;
    const borderColor = this.active
      ? (this.isHoveredOrFocused() ? 0xffffffff : 0xffa0a0a0)
      : 0xff606060;
    const fillColor = this.active ? 0xff101010 : 0xff202020;
    GuiComponent.fill(drawList, this.x, this.y, this.x + this.width, this.y + this.height, borderColor);
    GuiComponent.fill(drawList, this.x + 1, this.y + 1, this.x + this.width - 1, this.y + this.height - 1, fillColor);

    const text = this.displayText(this.width - 8);
    const textColor = this.active ? 0xffffffff : 0xffa0a0a0;
    GuiComponent.drawString(drawList, this.font, text, this.x + 4, this.y + Math.floor((this.height - 8) / 2), textColor);
  }

  private deleteBeforeCursor(): void {
    if (this.cursorPosition <= 0) {
      return;
    }

    this.value = `${this.value.slice(0, this.cursorPosition - 1)}${this.value.slice(this.cursorPosition)}`;
    this.cursorPosition--;
    this.responder(this.value);
  }

  private deleteAtCursor(): void {
    if (this.cursorPosition >= this.value.length) {
      return;
    }

    this.value = `${this.value.slice(0, this.cursorPosition)}${this.value.slice(this.cursorPosition + 1)}`;
    this.responder(this.value);
  }

  private displayText(maxWidth: number): string {
    const marker = this.isFocused() ? "|" : "";
    const rawValue = `${this.value.slice(0, this.cursorPosition)}${marker}${this.value.slice(this.cursorPosition)}`;
    const fullText = `${this.label}: ${rawValue}`;
    if (this.font.width(fullText) <= maxWidth) {
      return fullText;
    }

    const prefix = `${this.label}: ...`;
    let suffix = rawValue;
    while (suffix.length > 0 && this.font.width(`${prefix}${suffix}`) > maxWidth) {
      suffix = suffix.slice(1);
    }
    return `${prefix}${suffix}`;
  }

  private truncate(value: string): string {
    return value.length <= this.maxLength ? value : value.slice(0, this.maxLength);
  }
}

function isPrintableAscii(value: string): boolean {
  const codePoint = value.codePointAt(0) ?? 0;
  return codePoint >= 32 && codePoint <= 126;
}
