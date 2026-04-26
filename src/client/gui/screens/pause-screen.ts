import type { GuiDrawList } from "../../../renderer/gui/gui-draw-list";
import { Button } from "../components/button";
import { GuiComponent } from "../gui-component";
import { Screen } from "./screen";

export interface PauseScreenActions {
  onReturnToGame(): void;

  onOptions?(): void;

  onDebugSettings?(): void;

  onDisconnect?(): void;
}

export class PauseScreen extends Screen {
  public constructor(
    private readonly showPauseMenu: boolean,
    private readonly actions: PauseScreenActions,
  ) {
    super(showPauseMenu ? "menu.game" : "menu.paused");
  }

  protected override init(): void {
    if (this.showPauseMenu) {
      this.createPauseMenu();
    }
  }

  private createPauseMenu(): void {
    const rowOffset = -16;
    const baseY = Math.floor(this.height / 4);
    const centerX = Math.floor(this.width / 2);
    this.addRenderableWidget(new Button(centerX - 102, baseY + 24 + rowOffset, 204, 20, "Back to Game", this.getFont(), () => {
      this.actions.onReturnToGame();
    }));

    // WebGPU: downstream pause-menu screens are disabled until their GPU ports exist.
    const advancementsButton = this.addRenderableWidget(
      new Button(centerX - 102, baseY + 48 + rowOffset, 98, 20, "Advancements", this.getFont(), () => {}),
    );
    advancementsButton.active = false;
    const statsButton = this.addRenderableWidget(
      new Button(centerX + 4, baseY + 48 + rowOffset, 98, 20, "Stats", this.getFont(), () => {}),
    );
    statsButton.active = false;
    const feedbackButton = this.addRenderableWidget(
      new Button(centerX - 102, baseY + 72 + rowOffset, 98, 20, "Feedback", this.getFont(), () => {}),
    );
    feedbackButton.active = false;
    const bugsButton = this.addRenderableWidget(
      new Button(centerX + 4, baseY + 72 + rowOffset, 98, 20, "Report Bugs", this.getFont(), () => {}),
    );
    bugsButton.active = false;
    const optionsButton = this.addRenderableWidget(
      new Button(centerX - 102, baseY + 96 + rowOffset, 98, 20, "Options", this.getFont(), () => {
        this.actions.onOptions?.();
      }),
    );
    optionsButton.active = this.actions.onOptions !== undefined;
    // WebGPU: development-only debug settings occupy the unavailable LAN slot until LAN flow exists.
    const debugSettingsButton = this.addRenderableWidget(
      new Button(centerX + 4, baseY + 96 + rowOffset, 98, 20, "Debug Settings", this.getFont(), () => {
        this.actions.onDebugSettings?.();
      }),
    );
    debugSettingsButton.active = this.actions.onDebugSettings !== undefined;
    const disconnectButton = this.addRenderableWidget(
      new Button(centerX - 102, baseY + 120 + rowOffset, 204, 20, "Save and Quit to Title", this.getFont(), () => {
        this.actions.onDisconnect?.();
      }),
    );
    disconnectButton.active = this.actions.onDisconnect !== undefined;
  }

  public override render(drawList: GuiDrawList, mouseX: number, mouseY: number, partialTick: number): void {
    if (this.showPauseMenu) {
      this.renderBackground(drawList);
      GuiComponent.drawCenteredString(drawList, this.getFont(), "Game Menu", Math.floor(this.width / 2), 40, 0xffffffff);
    } else {
      GuiComponent.drawCenteredString(drawList, this.getFont(), "Game Paused", Math.floor(this.width / 2), 10, 0xffffffff);
    }

    super.render(drawList, mouseX, mouseY, partialTick);
  }
}
