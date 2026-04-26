import type { GuiDrawList } from "../../../renderer/gui/gui-draw-list";
import type { Font } from "../font";
import { GuiComponent } from "../gui-component";
import { AbstractButton } from "./abstract-button";

export type CheckboxOnChange = (checkbox: Checkbox, selected: boolean) => void;

export class Checkbox extends AbstractButton {
  public constructor(
    x: number,
    y: number,
    width: number,
    height: number,
    message: string,
    font: Font,
    private selectedValue: boolean,
    private readonly showLabel = true,
    private readonly onChange: CheckboxOnChange = () => {},
  ) {
    super(x, y, width, height, message, font);
  }

  public selected(): boolean {
    return this.selectedValue;
  }

  public override onPress(): void {
    this.selectedValue = !this.selectedValue;
    this.onChange(this, this.selectedValue);
  }

  public override renderButton(drawList: GuiDrawList, mouseX: number, mouseY: number, _partialTick: number): void {
    this.isHovered = mouseX >= this.x && mouseY >= this.y && mouseX < this.x + this.width && mouseY < this.y + this.height;
    const boxColor = this.active ? (this.isHoveredOrFocused() ? 0xffe0e0e0 : 0xffa0a0a0) : 0xff606060;
    const fillColor = this.active ? 0xff101010 : 0xff202020;
    // WebGPU: solid GPU quads stand in for checkbox.png until that texture is added to the GUI atlas.
    GuiComponent.fill(drawList, this.x, this.y, this.x + 20, this.y + 20, boxColor);
    GuiComponent.fill(drawList, this.x + 1, this.y + 1, this.x + 19, this.y + 19, fillColor);
    if (this.selectedValue) {
      GuiComponent.fill(drawList, this.x + 4, this.y + 10, this.x + 8, this.y + 14, 0xffffffff);
      GuiComponent.fill(drawList, this.x + 8, this.y + 13, this.x + 11, this.y + 17, 0xffffffff);
      GuiComponent.fill(drawList, this.x + 11, this.y + 7, this.x + 16, this.y + 11, 0xffffffff);
    }
    if (this.showLabel) {
      const textColor = this.active ? 0xffffffff : 0xffa0a0a0;
      GuiComponent.drawString(drawList, this.font, this.getMessage(), this.x + 24, this.y + 6, textColor);
    }
  }
}
