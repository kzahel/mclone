import type { GuiDrawList } from "../../../renderer/gui/gui-draw-list";
import type { Font } from "../font";
import { AbstractButton } from "./abstract-button";

export type ButtonOnPress = (button: Button) => void;
export type ButtonOnTooltip = (button: Button, drawList: GuiDrawList, mouseX: number, mouseY: number) => void;

export class Button extends AbstractButton {
  public static readonly NO_TOOLTIP: ButtonOnTooltip = () => {};

  public constructor(
    x: number,
    y: number,
    width: number,
    height: number,
    message: string,
    font: Font,
    protected readonly onPressCallback: ButtonOnPress,
    protected readonly onTooltip: ButtonOnTooltip = Button.NO_TOOLTIP,
  ) {
    super(x, y, width, height, message, font);
  }

  public override onPress(): void {
    this.onPressCallback(this);
  }

  public override renderButton(drawList: GuiDrawList, mouseX: number, mouseY: number, partialTick: number): void {
    super.renderButton(drawList, mouseX, mouseY, partialTick);
    if (this.isHoveredOrFocused()) {
      this.renderToolTip(drawList, mouseX, mouseY);
    }
  }

  public override renderToolTip(drawList: GuiDrawList, mouseX: number, mouseY: number): void {
    this.onTooltip(this, drawList, mouseX, mouseY);
  }
}
