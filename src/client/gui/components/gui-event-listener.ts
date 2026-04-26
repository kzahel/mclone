export interface GuiEventListener {
  mouseMoved?(mouseX: number, mouseY: number): void;

  mouseClicked?(mouseX: number, mouseY: number, button: number): boolean;

  mouseReleased?(mouseX: number, mouseY: number, button: number): boolean;

  mouseDragged?(mouseX: number, mouseY: number, button: number, dragX: number, dragY: number): boolean;

  mouseScrolled?(mouseX: number, mouseY: number, delta: number): boolean;

  keyPressed?(keyCode: number, scanCode: number, modifiers: number): boolean;

  keyReleased?(keyCode: number, scanCode: number, modifiers: number): boolean;

  charTyped?(codePoint: string, modifiers: number): boolean;

  changeFocus?(forward: boolean): boolean;

  isMouseOver?(mouseX: number, mouseY: number): boolean;
}
