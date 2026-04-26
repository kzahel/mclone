import type { GuiDrawList } from "../../../renderer/gui/gui-draw-list";
import { Button } from "../components/button";
import { GuiComponent } from "../gui-component";
import { Screen } from "./screen";

export class AlertScreen extends Screen {
  public constructor(
    private readonly callback: () => void,
    title: string,
    protected readonly text: string,
    protected readonly okButton = "Back",
  ) {
    super(title);
  }

  protected override init(): void {
    super.init();
    this.addRenderableWidget(new Button(
      Math.floor(this.width / 2) - 100,
      Math.min(Math.floor(this.height / 6) + 168, this.height - 30),
      200,
      20,
      this.okButton,
      this.getFont(),
      () => this.callback(),
    ));
  }

  public override render(drawList: GuiDrawList, mouseX: number, mouseY: number, partialTick: number): void {
    this.renderBackground(drawList);
    GuiComponent.drawCenteredString(drawList, this.getFont(), this.getTitle(), Math.floor(this.width / 2), 70, 0xffffffff);
    this.renderCenteredMessage(drawList);
    super.render(drawList, mouseX, mouseY, partialTick);
  }

  private renderCenteredMessage(drawList: GuiDrawList): void {
    const lines = wrapText(this.text, Math.max(40, this.width - 50), (text) => this.getFont().width(text));
    let y = 90;
    for (const line of lines) {
      GuiComponent.drawCenteredString(drawList, this.getFont(), line, Math.floor(this.width / 2), y, 0xffffffff);
      y += this.getFont().lineHeight;
    }
  }
}

function wrapText(text: string, maxWidth: number, measure: (text: string) => number): string[] {
  const lines: string[] = [];
  for (const paragraph of text.split("\n")) {
    let line = "";
    for (const word of paragraph.split(" ")) {
      const next = line.length === 0 ? word : `${line} ${word}`;
      if (line.length > 0 && measure(next) > maxWidth) {
        lines.push(line);
        line = word;
      } else {
        line = next;
      }
    }
    if (line.length > 0) {
      lines.push(line);
    }
  }
  return lines.length === 0 ? [""] : lines;
}
