import { GuiDrawList } from "../../../renderer/gui/gui-draw-list";
import { ContainerEventHandler } from "../components/container-event-handler";
import type { GuiEventListener } from "../components/gui-event-listener";
import type { Widget } from "../components/widget";
import type { Font } from "../font";
import { GuiComponent } from "../gui-component";
import type { GuiRenderMetrics } from "../gui-render-metrics";
import type { ScreenManager } from "../screen-manager";

export abstract class Screen extends ContainerEventHandler implements Widget {
  protected manager: ScreenManager | null = null;
  public width = 0;
  public height = 0;
  public passEvents = false;
  protected font: Font | null = null;
  private readonly childWidgets: GuiEventListener[] = [];
  private readonly renderables: Widget[] = [];

  protected constructor(protected readonly title: string) {
    super();
  }

  public getTitle(): string {
    return this.title;
  }

  public getNarrationMessage(): string {
    return this.getTitle();
  }

  public render(
    drawList: GuiDrawList,
    mouseX: number,
    mouseY: number,
    partialTick: number,
    _metrics?: GuiRenderMetrics,
  ): void {
    for (const renderable of this.renderables) {
      renderable.render(drawList, mouseX, mouseY, partialTick);
    }
  }

  public override keyPressed(keyCode: number, scanCode: number, modifiers: number): boolean {
    if (keyCode === 256 && this.shouldCloseOnEsc()) {
      this.onClose();
      return true;
    }
    if (keyCode === 258) {
      const forward = true;
      if (!this.changeFocus(forward)) {
        this.changeFocus(forward);
      }
      return false;
    }
    return super.keyPressed(keyCode, scanCode, modifiers);
  }

  public shouldCloseOnEsc(): boolean {
    return true;
  }

  public onClose(): void {
    this.manager?.setScreen(null);
  }

  protected addRenderableWidget<T extends GuiEventListener & Widget>(widget: T): T {
    this.renderables.push(widget);
    return this.addWidget(widget);
  }

  protected addRenderableOnly<T extends Widget>(widget: T): T {
    this.renderables.push(widget);
    return widget;
  }

  protected addWidget<T extends GuiEventListener>(widget: T): T {
    this.childWidgets.push(widget);
    return widget;
  }

  protected removeWidget(widget: GuiEventListener): void {
    this.removeFromArray(this.childWidgets, widget);
    if (isWidget(widget)) {
      this.removeFromArray(this.renderables, widget);
    }
  }

  protected clearWidgets(): void {
    this.renderables.length = 0;
    this.childWidgets.length = 0;
  }

  public initialize(manager: ScreenManager, width: number, height: number, font: Font): void {
    this.manager = manager;
    this.font = font;
    this.width = width;
    this.height = height;
    this.clearWidgets();
    this.setFocused(null);
    this.init();
  }

  public override children(): readonly GuiEventListener[] {
    return this.childWidgets;
  }

  protected init(): void {}

  public tick(): void {}

  public removed(): void {}

  public renderBackground(drawList: GuiDrawList): void {
    this.renderBackgroundWithVOffset(drawList, 0);
  }

  public renderBackgroundWithVOffset(drawList: GuiDrawList, _vOffset: number): void {
    this.fillGradient(drawList, 0, 0, this.width, this.height, 0xc0101010, 0xd0101010);
  }

  public renderDirtBackground(drawList: GuiDrawList, _vOffset: number): void {
    // WebGPU: temporary solid background until the GUI dirt texture is added to the atlas.
    GuiComponent.fill(drawList, 0, 0, this.width, this.height, 0xff202020);
  }

  public isPauseScreen(): boolean {
    return true;
  }

  public override isMouseOver(_mouseX: number, _mouseY: number): boolean {
    return true;
  }

  protected getFont(): Font {
    if (this.font === null) {
      throw new Error("Screen font used before initialization");
    }
    return this.font;
  }

  private removeFromArray<T>(array: T[], value: T): void {
    const index = array.indexOf(value);
    if (index >= 0) {
      array.splice(index, 1);
    }
  }
}

function isWidget(value: GuiEventListener): value is GuiEventListener & Widget {
  return typeof (value as Partial<Widget>).render === "function";
}
