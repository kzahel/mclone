import type { GuiDrawList } from "../../../renderer/gui/gui-draw-list";
import { Button } from "../components/button";
import { GuiComponent } from "../gui-component";
import { Screen } from "./screen";

export interface TitleScreenActions {
  onContinue(): void;

  onStartWorld(): void;

  onOptions(): void;

  onDebugSettings(): void;
}

export class TitleScreen extends Screen {
  private readonly fading = false;
  private statusMessage = "";

  public constructor(private readonly actions: TitleScreenActions) {
    super("narrator.screen.title");
  }

  public override shouldCloseOnEsc(): boolean {
    return false;
  }

  public override isPauseScreen(): boolean {
    return false;
  }

  protected override init(): void {
    const spacing = 24;
    const top = Math.floor(this.height / 4) + 48;
    this.addRenderableWidget(new Button(this.width / 2 - 100, top, 200, 20, "Continue", this.getFont(), () => {
      this.statusMessage = "Continue selected";
      this.actions.onContinue();
    }));
    this.addRenderableWidget(new Button(this.width / 2 - 100, top + spacing, 200, 20, "Start World", this.getFont(), () => {
      this.statusMessage = "Start World selected";
      this.actions.onStartWorld();
    }));
    this.addRenderableWidget(new Button(this.width / 2 - 100, top + (spacing * 2), 200, 20, "Options", this.getFont(), () => {
      this.statusMessage = "Options selected";
      this.actions.onOptions();
    }));
    this.addRenderableWidget(new Button(this.width / 2 - 100, top + (spacing * 3), 200, 20, "Debug Settings", this.getFont(), () => {
      this.statusMessage = "Debug Settings selected";
      this.actions.onDebugSettings();
    }));
  }

  public override render(drawList: GuiDrawList, mouseX: number, mouseY: number, partialTick: number): void {
    // WebGPU: simple GPU gradient/title text in place of vanilla panorama, logo, splash, and Realms widgets.
    GuiComponent.fillGradient(drawList, 0, 0, this.width, this.height, 0xff26354a, 0xff101318);
    GuiComponent.fillGradient(drawList, 0, 0, this.width, this.height, 0x30000000, 0xb0000000);
    GuiComponent.drawCenteredString(drawList, this.getFont(), "mclone", Math.floor(this.width / 2), 42, 0xffffffff);
    GuiComponent.drawCenteredString(drawList, this.getFont(), "Java 1.17.1 WebGPU client", Math.floor(this.width / 2), 60, 0xffffff55);
    super.render(drawList, mouseX, mouseY, partialTick);
    if (this.statusMessage.length > 0) {
      GuiComponent.drawCenteredString(drawList, this.getFont(), this.statusMessage, Math.floor(this.width / 2), Math.floor(this.height / 4) + 132, 0xffa0ffa0);
    }
    const versionText = "Minecraft 1.17.1 target";
    const copyright = "Copyright Mojang AB.";
    const distribution = "Do not distribute!";
    const rightMargin = 8;
    const versionWidth = this.getFont().width(versionText);
    const copyrightWidth = this.getFont().width(`${copyright} ${distribution}`);
    if (versionWidth + copyrightWidth + rightMargin + 4 <= this.width) {
      GuiComponent.drawString(drawList, this.getFont(), versionText, 2, this.height - 10, 0xffffffff);
      GuiComponent.drawString(drawList, this.getFont(), `${copyright} ${distribution}`, this.width - copyrightWidth - rightMargin, this.height - 10, 0xffffffff);
    } else {
      GuiComponent.drawString(drawList, this.getFont(), versionText, 2, this.height - 30, 0xffffffff);
      GuiComponent.drawString(drawList, this.getFont(), copyright, this.width - this.getFont().width(copyright) - rightMargin, this.height - 20, 0xffffffff);
      GuiComponent.drawString(drawList, this.getFont(), distribution, this.width - this.getFont().width(distribution) - rightMargin, this.height - 10, 0xffffffff);
    }
  }

  public isFading(): boolean {
    return this.fading;
  }
}
