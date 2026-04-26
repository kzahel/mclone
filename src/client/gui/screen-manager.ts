import { GuiDrawList } from "../../renderer/gui/gui-draw-list";
import { Font } from "./font";
import type { Screen } from "./screens/screen";

export class ScreenManager {
  public currentScreen: Screen | null = null;
  public readonly font: Font;
  private width: number;
  private height: number;

  public constructor(width: number, height: number, font = new Font()) {
    this.width = width;
    this.height = height;
    this.font = font;
  }

  public setScreen(screen: Screen | null): void {
    this.currentScreen?.removed();
    this.currentScreen = screen;
    if (screen !== null) {
      screen.initialize(this, this.width, this.height, this.font);
    }
  }

  public resize(width: number, height: number): void {
    if (this.width === width && this.height === height) {
      return;
    }

    this.width = width;
    this.height = height;
    this.currentScreen?.initialize(this, width, height, this.font);
  }

  public tick(): void {
    this.currentScreen?.tick();
  }

  public render(drawList: GuiDrawList, mouseX: number, mouseY: number, partialTick: number): void {
    this.currentScreen?.render(drawList, mouseX, mouseY, partialTick);
  }

  public renderToDrawList(mouseX: number, mouseY: number, partialTick: number): GuiDrawList {
    const drawList = new GuiDrawList();
    this.render(drawList, mouseX, mouseY, partialTick);
    return drawList;
  }

  public mouseMoved(mouseX: number, mouseY: number): void {
    this.currentScreen?.mouseMoved?.(mouseX, mouseY);
  }

  public mouseClicked(mouseX: number, mouseY: number, button: number): boolean {
    return this.currentScreen?.mouseClicked(mouseX, mouseY, button) ?? false;
  }

  public mouseReleased(mouseX: number, mouseY: number, button: number): boolean {
    return this.currentScreen?.mouseReleased(mouseX, mouseY, button) ?? false;
  }

  public mouseDragged(mouseX: number, mouseY: number, button: number, dragX: number, dragY: number): boolean {
    return this.currentScreen?.mouseDragged(mouseX, mouseY, button, dragX, dragY) ?? false;
  }

  public mouseScrolled(mouseX: number, mouseY: number, delta: number): boolean {
    return this.currentScreen?.mouseScrolled(mouseX, mouseY, delta) ?? false;
  }

  public keyPressed(keyCode: number, scanCode: number, modifiers: number): boolean {
    return this.currentScreen?.keyPressed(keyCode, scanCode, modifiers) ?? false;
  }

  public keyReleased(keyCode: number, scanCode: number, modifiers: number): boolean {
    return this.currentScreen?.keyReleased(keyCode, scanCode, modifiers) ?? false;
  }

  public charTyped(codePoint: string, modifiers: number): boolean {
    return this.currentScreen?.charTyped(codePoint, modifiers) ?? false;
  }

  public getWidth(): number {
    return this.width;
  }

  public getHeight(): number {
    return this.height;
  }
}
