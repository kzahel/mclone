import type { OpenWorldPreset, WorldStorageMode } from "../../../runtime/protocol/world-messages";
import type { GuiDrawList } from "../../../renderer/gui/gui-draw-list";
import { Button } from "../components/button";
import { Checkbox } from "../components/checkbox";
import { CycleButton } from "../components/cycle-button";
import { GuiComponent } from "../gui-component";
import { Screen } from "./screen";

export type GuiDebugMovementMode = "player" | "freecam";

export interface GuiDebugSettingsState {
  movementMode: GuiDebugMovementMode;
  preset: OpenWorldPreset;
  worldStorageMode: WorldStorageMode;
  showDebugInfo: boolean;
}

export interface DebugSettingsScreenActions {
  onChanged?(state: GuiDebugSettingsState): void;

  onDone?(): void;
}

export class DebugSettingsScreen extends Screen {
  public constructor(
    private readonly lastScreen: Screen,
    private readonly settings: GuiDebugSettingsState,
    private readonly actions: DebugSettingsScreenActions = {},
  ) {
    super("debug.settings.title");
  }

  protected override init(): void {
    const centerX = Math.floor(this.width / 2);
    const leftX = centerX - 155;
    const rightX = centerX + 5;
    const topY = Math.floor(this.height / 6) - 12;
    const rowSpacing = 24;
    const doneY = Math.min(this.height - 28, topY + (rowSpacing * 4) + 12);

    this.addRenderableWidget(new CycleButton<GuiDebugMovementMode>(
      leftX,
      topY,
      150,
      20,
      "Movement",
      this.getFont(),
      ["player", "freecam"],
      this.settings.movementMode,
      formatMovementMode,
      (_button, value) => {
        this.settings.movementMode = value;
        this.onChanged();
      },
    ));
    this.addRenderableWidget(new CycleButton<OpenWorldPreset>(
      rightX,
      topY,
      150,
      20,
      "Preset",
      this.getFont(),
      ["browser_smoke", "default", "flat_grass", "small_island"],
      this.settings.preset,
      formatPreset,
      (_button, value) => {
        this.settings.preset = value;
        this.onChanged();
      },
    ));
    this.addRenderableWidget(new CycleButton<WorldStorageMode>(
      leftX,
      topY + rowSpacing,
      150,
      20,
      "Storage",
      this.getFont(),
      ["default", "none"],
      this.settings.worldStorageMode,
      formatWorldStorageMode,
      (_button, value) => {
        this.settings.worldStorageMode = value;
        this.onChanged();
      },
    ));
    this.addRenderableWidget(new Checkbox(
      rightX,
      topY + rowSpacing,
      150,
      20,
      "Show Debug Info",
      this.getFont(),
      this.settings.showDebugInfo,
      true,
      (_checkbox, selected) => {
        this.settings.showDebugInfo = selected;
        this.onChanged();
      },
    ));
    this.addRenderableWidget(new Button(centerX - 100, doneY, 200, 20, "Done", this.getFont(), () => this.onClose()));
  }

  public override onClose(): void {
    this.manager?.setScreen(this.lastScreen);
    this.actions.onDone?.();
  }

  public override removed(): void {
    this.onChanged();
  }

  public override render(drawList: GuiDrawList, mouseX: number, mouseY: number, partialTick: number): void {
    this.renderBackground(drawList);
    GuiComponent.drawCenteredString(drawList, this.getFont(), "Debug Settings", Math.floor(this.width / 2), 15, 0xffffffff);
    super.render(drawList, mouseX, mouseY, partialTick);
  }

  private onChanged(): void {
    this.actions.onChanged?.(this.settings);
  }
}

function formatMovementMode(value: GuiDebugMovementMode): string {
  return value === "freecam" ? "Free Cam" : "Player";
}

function formatPreset(value: OpenWorldPreset): string {
  switch (value) {
    case "browser_smoke":
      return "Browser Smoke";
    case "flat_grass":
      return "Flat Grass";
    case "small_island":
      return "Small Island";
    case "default":
      return "Default";
  }
}

function formatWorldStorageMode(value: WorldStorageMode): string {
  return value === "none" ? "Off" : "IndexedDB";
}
