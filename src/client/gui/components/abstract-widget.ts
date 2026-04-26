import { GuiComponent } from "../gui-component";
import type { Font } from "../font";
import type { GuiEventListener } from "./gui-event-listener";
import type { Widget } from "./widget";
import type { GuiDrawList } from "../../../renderer/gui/gui-draw-list";

export abstract class AbstractWidget extends GuiComponent implements Widget, GuiEventListener {
  public active = true;
  public visible = true;
  protected isHovered = false;
  protected alpha = 1.0;
  private focused = false;

  public constructor(
    public x: number,
    public y: number,
    protected width: number,
    protected height: number,
    private message: string,
    protected readonly font: Font,
  ) {
    super();
  }

  public getHeight(): number {
    return this.height;
  }

  protected getYImage(hovered: boolean): number {
    if (!this.active) {
      return 0;
    }
    return hovered ? 2 : 1;
  }

  public render(drawList: GuiDrawList, mouseX: number, mouseY: number, partialTick: number): void {
    if (this.visible) {
      this.isHovered = mouseX >= this.x && mouseY >= this.y && mouseX < this.x + this.width && mouseY < this.y + this.height;
      this.renderButton(drawList, mouseX, mouseY, partialTick);
    }
  }

  public renderButton(drawList: GuiDrawList, mouseX: number, mouseY: number, _partialTick: number): void {
    const yImage = this.getYImage(this.isHoveredOrFocused());
    const alphaBits = Math.ceil(this.alpha * 255.0) << 24;
    this.blit(drawList, this.x, this.y, 0, 46 + (yImage * 20), Math.floor(this.width / 2), this.height);
    this.blit(
      drawList,
      this.x + Math.floor(this.width / 2),
      this.y,
      200 - Math.ceil(this.width / 2),
      46 + (yImage * 20),
      Math.ceil(this.width / 2),
      this.height,
    );
    this.renderBg(drawList, mouseX, mouseY);
    const textColor = this.active ? 0x00ffffff : 0x00a0a0a0;
    GuiComponent.drawCenteredString(
      drawList,
      this.font,
      this.getMessage(),
      this.x + Math.floor(this.width / 2),
      this.y + Math.floor((this.height - 8) / 2),
      (textColor | alphaBits) >>> 0,
    );
  }

  protected renderBg(_drawList: GuiDrawList, _mouseX: number, _mouseY: number): void {}

  public onClick(_mouseX: number, _mouseY: number): void {}

  public onRelease(_mouseX: number, _mouseY: number): void {}

  protected onDrag(_mouseX: number, _mouseY: number, _dragX: number, _dragY: number): void {}

  public mouseClicked(mouseX: number, mouseY: number, button: number): boolean {
    if (this.active && this.visible && this.isValidClickButton(button) && this.clicked(mouseX, mouseY)) {
      this.playDownSound();
      this.onClick(mouseX, mouseY);
      return true;
    }
    return false;
  }

  public mouseReleased(mouseX: number, mouseY: number, button: number): boolean {
    if (this.isValidClickButton(button)) {
      this.onRelease(mouseX, mouseY);
      return true;
    }
    return false;
  }

  public mouseDragged(mouseX: number, mouseY: number, button: number, dragX: number, dragY: number): boolean {
    if (this.isValidClickButton(button)) {
      this.onDrag(mouseX, mouseY, dragX, dragY);
      return true;
    }
    return false;
  }

  protected isValidClickButton(button: number): boolean {
    return button === 0;
  }

  protected clicked(mouseX: number, mouseY: number): boolean {
    return this.active && this.visible && mouseX >= this.x && mouseY >= this.y && mouseX < this.x + this.width && mouseY < this.y + this.height;
  }

  public isHoveredOrFocused(): boolean {
    return this.isHovered || this.focused;
  }

  public changeFocus(_forward: boolean): boolean {
    if (this.active && this.visible) {
      this.focused = !this.focused;
      this.onFocusedChanged(this.focused);
      return this.focused;
    }
    return false;
  }

  protected onFocusedChanged(_focused: boolean): void {}

  public isMouseOver(mouseX: number, mouseY: number): boolean {
    return this.active && this.visible && mouseX >= this.x && mouseY >= this.y && mouseX < this.x + this.width && mouseY < this.y + this.height;
  }

  public renderToolTip(_drawList: GuiDrawList, _mouseX: number, _mouseY: number): void {}

  public playDownSound(): void {
    // WebGPU: no client sound manager exists for GUI widgets yet.
  }

  public getWidth(): number {
    return this.width;
  }

  public setWidth(width: number): void {
    this.width = width;
  }

  public setAlpha(alpha: number): void {
    this.alpha = alpha;
  }

  public setMessage(message: string): void {
    this.message = message;
  }

  public getMessage(): string {
    return this.message;
  }

  public isFocused(): boolean {
    return this.focused;
  }

  public isActive(): boolean {
    return this.visible && this.active;
  }

  protected setFocused(focused: boolean): void {
    this.focused = focused;
  }
}
