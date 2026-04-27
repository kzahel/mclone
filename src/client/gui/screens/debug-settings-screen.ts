import type { OpenWorldPreset, WorldStorageMode } from "../../../runtime/protocol/world-messages";
import type { GuiDrawList } from "../../../renderer/gui/gui-draw-list";
import { Button } from "../components/button";
import { Checkbox } from "../components/checkbox";
import { CycleButton } from "../components/cycle-button";
import { EditBox } from "../components/edit-box";
import { GuiComponent } from "../gui-component";
import { Screen } from "./screen";

export type GuiDebugMovementMode = "player" | "freecam";
export type GuiWorldAuthority = "local" | "dedicated";

export interface GuiDebugSettingsState {
  movementMode: GuiDebugMovementMode;
  preset: OpenWorldPreset;
  worldStorageMode: WorldStorageMode;
  showDebugInfo: boolean;
  worldAuthority: GuiWorldAuthority;
  dedicatedSocketUrl: string;
}

export interface DebugSettingsScreenActions {
  onChanged?(state: GuiDebugSettingsState): void;

  onDone?(): void;
}

export class DebugSettingsScreen extends Screen {
  private dedicatedSocketUrlBox: EditBox | undefined;

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
    this.addRenderableWidget(new CycleButton<GuiWorldAuthority>(
      leftX,
      topY + (rowSpacing * 2),
      310,
      20,
      "World",
      this.getFont(),
      ["local", "dedicated"],
      this.settings.worldAuthority,
      formatWorldAuthority,
      (_button, value) => {
        this.settings.worldAuthority = value;
        this.updateDedicatedSocketUrlBox();
        this.onChanged();
      },
    ));
    this.dedicatedSocketUrlBox = this.addRenderableWidget(new EditBox(
      leftX,
      topY + (rowSpacing * 3),
      310,
      20,
      "Server",
      this.getFont(),
      this.settings.dedicatedSocketUrl,
      160,
      (value) => {
        this.settings.dedicatedSocketUrl = value;
        this.onChanged();
      },
    ));
    this.updateDedicatedSocketUrlBox();
    this.addRenderableWidget(new Button(centerX - 100, doneY, 200, 20, "Done", this.getFont(), () => this.onClose()));
  }

  public override mouseClicked(mouseX: number, mouseY: number, button: number): boolean {
    const handled = super.mouseClicked(mouseX, mouseY, button);
    if (this.dedicatedSocketUrlBox !== undefined && !this.dedicatedSocketUrlBox.isMouseOver(mouseX, mouseY)) {
      this.dedicatedSocketUrlBox.setFocusedForInput(false);
    }
    return handled;
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

  private updateDedicatedSocketUrlBox(): void {
    if (this.dedicatedSocketUrlBox !== undefined) {
      this.dedicatedSocketUrlBox.active = this.settings.worldAuthority === "dedicated";
    }
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

function formatWorldAuthority(value: GuiWorldAuthority): string {
  return value === "dedicated" ? "Dedicated Server" : "Local Singleplayer";
}
