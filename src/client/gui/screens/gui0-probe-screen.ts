import type { GuiDrawList } from "../../../renderer/gui/gui-draw-list";
import { Button } from "../components/button";
import { GuiComponent } from "../gui-component";
import { Screen } from "./screen";

const BUTTON_WIDTH = 200;
const BUTTON_HEIGHT = 20;
const PROGRESS_WIDTH = 220;
const PROGRESS_HEIGHT = 10;

export class Gui0ProbeScreen extends Screen {
  private pressCount = 0;
  private progress = 0.68;

  public constructor() {
    super("Gui0 Probe");
  }

  protected override init(): void {
    const buttonX = Math.floor((this.width - BUTTON_WIDTH) / 2);
    const buttonY = Math.floor(this.height / 2) - 4;
    this.addRenderableWidget(new Button(
      buttonX,
      buttonY,
      BUTTON_WIDTH,
      BUTTON_HEIGHT,
      "Gui0 Button",
      this.getFont(),
      () => {
        this.pressCount++;
        this.progress = this.progress >= 0.98 ? 0.15 : this.progress + 0.1;
      },
    ));
  }

  public override render(drawList: GuiDrawList, mouseX: number, mouseY: number, partialTick: number): void {
    this.renderBackground(drawList);
    GuiComponent.drawCenteredString(drawList, this.getFont(), "mclone", Math.floor(this.width / 2), 32, 0xffffffff);
    GuiComponent.drawCenteredString(drawList, this.getFont(), "WebGPU GUI foundation", Math.floor(this.width / 2), 48, 0xffe0e0e0);
    super.render(drawList, mouseX, mouseY, partialTick);
    this.renderProgressBar(drawList);
    GuiComponent.drawCenteredString(
      drawList,
      this.getFont(),
      `Progress ${Math.round(this.progress * 100).toString()}%`,
      Math.floor(this.width / 2),
      Math.floor(this.height / 2) + 36,
      0xffffffff,
    );
    if (this.pressCount > 0) {
      GuiComponent.drawCenteredString(
        drawList,
        this.getFont(),
        `Pressed ${this.pressCount.toString()}`,
        Math.floor(this.width / 2),
        Math.floor(this.height / 2) + 52,
        0xffa0ffa0,
      );
    }
  }

  private renderProgressBar(drawList: GuiDrawList): void {
    const x = Math.floor((this.width - PROGRESS_WIDTH) / 2);
    const y = Math.floor(this.height / 2) + 23;
    GuiComponent.fill(drawList, x - 1, y - 1, x + PROGRESS_WIDTH + 1, y + PROGRESS_HEIGHT + 1, 0xff000000);
    GuiComponent.fill(drawList, x, y, x + PROGRESS_WIDTH, y + PROGRESS_HEIGHT, 0xff555555);
    GuiComponent.fill(drawList, x, y, x + Math.floor(PROGRESS_WIDTH * this.progress), y + PROGRESS_HEIGHT, 0xff80ff20);
    GuiComponent.fill(drawList, x, y, x + PROGRESS_WIDTH, y + 1, 0xffa0a0a0);
  }
}
