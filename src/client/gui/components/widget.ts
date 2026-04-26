import type { GuiDrawList } from "../../../renderer/gui/gui-draw-list";

export interface Widget {
  render(drawList: GuiDrawList, mouseX: number, mouseY: number, partialTick: number): void;
}
