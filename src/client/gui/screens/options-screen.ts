import type { WorldEngineLightingMode, WorldEngineLiquidSimulationMode } from "../../../runtime/protocol/world-messages";
import type { GuiDrawList } from "../../../renderer/gui/gui-draw-list";
import { AbstractSliderButton } from "../components/abstract-slider-button";
import { Button } from "../components/button";
import { Checkbox } from "../components/checkbox";
import { CycleButton } from "../components/cycle-button";
import type { Font } from "../font";
import { GuiComponent } from "../gui-component";
import { Screen } from "./screen";

export interface GuiOptionsState {
  viewDistance: number;
  renderDistance: number;
  lightingMode: WorldEngineLightingMode;
  liquidSimulationMode: WorldEngineLiquidSimulationMode;
}

export interface OptionsScreenActions {
  onChanged?(state: GuiOptionsState): void;

  onDone?(): void;
}

export class OptionsScreen extends Screen {
  public constructor(
    private readonly lastScreen: Screen,
    private readonly options: GuiOptionsState,
    private readonly actions: OptionsScreenActions = {},
  ) {
    super("options.title");
  }

  protected override init(): void {
    const centerX = Math.floor(this.width / 2);
    const leftX = centerX - 155;
    const rightX = centerX + 5;
    const topY = Math.floor(this.height / 6) - 12;
    const rowSpacing = 24;
    const doneY = Math.min(this.height - 28, topY + (rowSpacing * 4) + 12);

    this.addRenderableWidget(new IntegerSliderButton(
      leftX,
      topY,
      150,
      20,
      "View",
      this.getFont(),
      this.options,
      "viewDistance",
      1,
      16,
      1,
      " chunks",
      () => this.onChanged(),
    ));
    this.addRenderableWidget(new IntegerSliderButton(
      rightX,
      topY,
      150,
      20,
      "Render",
      this.getFont(),
      this.options,
      "renderDistance",
      16,
      512,
      16,
      " blocks",
      () => this.onChanged(),
    ));
    this.addRenderableWidget(new CycleButton<WorldEngineLightingMode>(
      leftX,
      topY + rowSpacing,
      150,
      20,
      "Lighting",
      this.getFont(),
      ["vanilla17", "none"],
      this.options.lightingMode,
      formatLightingMode,
      (_button, value) => {
        this.options.lightingMode = value;
        this.onChanged();
      },
    ));
    this.addRenderableWidget(new Checkbox(
      rightX,
      topY + rowSpacing,
      150,
      20,
      "Water Simulation",
      this.getFont(),
      this.options.liquidSimulationMode !== "none",
      true,
      (_checkbox, selected) => {
        this.options.liquidSimulationMode = selected ? "vanilla17" : "none";
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
    GuiComponent.drawCenteredString(drawList, this.getFont(), "Options", Math.floor(this.width / 2), 15, 0xffffffff);
    super.render(drawList, mouseX, mouseY, partialTick);
  }

  private onChanged(): void {
    this.actions.onChanged?.(this.options);
  }
}

type IntegerOptionKey = "viewDistance" | "renderDistance";

class IntegerSliderButton extends AbstractSliderButton {
  public constructor(
    x: number,
    y: number,
    width: number,
    height: number,
    private readonly label: string,
    font: Font,
    private readonly options: Pick<GuiOptionsState, IntegerOptionKey>,
    private readonly key: IntegerOptionKey,
    private readonly minValue: number,
    private readonly maxValue: number,
    private readonly step: number,
    private readonly suffix: string,
    private readonly onValueChanged: () => void,
  ) {
    super(x, y, width, height, "", font, normalizeValue(options[key], minValue, maxValue));
    this.updateMessage();
  }

  protected override updateMessage(): void {
    this.setMessage(`${this.label}: ${this.valueFromSlider()}${this.suffix}`);
  }

  protected override applyValue(): void {
    this.options[this.key] = this.valueFromSlider();
    this.onValueChanged();
  }

  private valueFromSlider(): number {
    const range = this.maxValue - this.minValue;
    const stepped = this.minValue + (Math.round((this.value * range) / this.step) * this.step);
    return Math.min(this.maxValue, Math.max(this.minValue, stepped));
  }
}

function normalizeValue(value: number, minValue: number, maxValue: number): number {
  if (maxValue <= minValue) {
    return 0;
  }
  return Math.min(1, Math.max(0, (value - minValue) / (maxValue - minValue)));
}

function formatLightingMode(value: WorldEngineLightingMode): string {
  return value === "vanilla17" ? "Vanilla 1.17" : "Off";
}
