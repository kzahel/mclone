import { GuiComponent } from "../gui-component";
import type { GuiEventListener } from "./gui-event-listener";

export abstract class ContainerEventHandler extends GuiComponent implements GuiEventListener {
  private focused: GuiEventListener | null = null;
  private dragging = false;

  public abstract children(): readonly GuiEventListener[];

  public mouseMoved(mouseX: number, mouseY: number): void {
    this.getChildAt(mouseX, mouseY)?.mouseMoved?.(mouseX, mouseY);
  }

  public getChildAt(mouseX: number, mouseY: number): GuiEventListener | undefined {
    return this.children().find((child) => child.isMouseOver?.(mouseX, mouseY) ?? false);
  }

  public mouseClicked(mouseX: number, mouseY: number, button: number): boolean {
    for (const child of this.children()) {
      if (child.mouseClicked?.(mouseX, mouseY, button) === true) {
        this.setFocused(child);
        if (button === 0) {
          this.setDragging(true);
        }
        return true;
      }
    }
    return false;
  }

  public mouseReleased(mouseX: number, mouseY: number, button: number): boolean {
    this.setDragging(false);
    return this.getChildAt(mouseX, mouseY)?.mouseReleased?.(mouseX, mouseY, button) ?? false;
  }

  public mouseDragged(mouseX: number, mouseY: number, button: number, dragX: number, dragY: number): boolean {
    return this.getFocused() !== null && this.isDragging() && button === 0
      ? this.getFocused()!.mouseDragged?.(mouseX, mouseY, button, dragX, dragY) ?? false
      : false;
  }

  public mouseScrolled(mouseX: number, mouseY: number, delta: number): boolean {
    return this.getChildAt(mouseX, mouseY)?.mouseScrolled?.(mouseX, mouseY, delta) ?? false;
  }

  public keyPressed(keyCode: number, scanCode: number, modifiers: number): boolean {
    return this.getFocused()?.keyPressed?.(keyCode, scanCode, modifiers) ?? false;
  }

  public keyReleased(keyCode: number, scanCode: number, modifiers: number): boolean {
    return this.getFocused()?.keyReleased?.(keyCode, scanCode, modifiers) ?? false;
  }

  public charTyped(codePoint: string, modifiers: number): boolean {
    return this.getFocused()?.charTyped?.(codePoint, modifiers) ?? false;
  }

  public isMouseOver(_mouseX: number, _mouseY: number): boolean {
    return false;
  }

  public isDragging(): boolean {
    return this.dragging;
  }

  public setDragging(dragging: boolean): void {
    this.dragging = dragging;
  }

  public getFocused(): GuiEventListener | null {
    return this.focused;
  }

  public setFocused(focused: GuiEventListener | null): void {
    this.focused = focused;
  }

  public setInitialFocus(focused: GuiEventListener | null): void {
    this.setFocused(focused);
    focused?.changeFocus?.(true);
  }

  public changeFocus(forward: boolean): boolean {
    const focused = this.getFocused();
    const hadFocused = focused !== null;
    if (hadFocused && focused.changeFocus?.(forward) === true) {
      return true;
    }

    const children = this.children();
    const focusedIndex = children.indexOf(focused as GuiEventListener);
    let index = 0;
    if (hadFocused && focusedIndex >= 0) {
      index = focusedIndex + (forward ? 1 : 0);
    } else if (!forward) {
      index = children.length;
    }

    const step = forward ? 1 : -1;
    for (let cursor = index; cursor >= 0 && cursor < children.length; cursor += step) {
      const child = children[cursor]!;
      if (child.changeFocus?.(forward) === true) {
        this.setFocused(child);
        return true;
      }
    }

    this.setFocused(null);
    return false;
  }
}
