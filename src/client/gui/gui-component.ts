import type { GuiDrawList, GuiTextureId } from "../../renderer/gui/gui-draw-list";
import type { Font } from "./font";

export class GuiComponent {
  private blitOffset = 0;

  protected hLine(drawList: GuiDrawList, x0: number, x1: number, y: number, color: number): void {
    if (x1 < x0) {
      const swap = x0;
      x0 = x1;
      x1 = swap;
    }
    GuiComponent.fill(drawList, x0, y, x1 + 1, y + 1, color);
  }

  protected vLine(drawList: GuiDrawList, x: number, y0: number, y1: number, color: number): void {
    if (y1 < y0) {
      const swap = y0;
      y0 = y1;
      y1 = swap;
    }
    GuiComponent.fill(drawList, x, y0 + 1, x + 1, y1, color);
  }

  public static fill(drawList: GuiDrawList, x0: number, y0: number, x1: number, y1: number, color: number): void {
    // WebGPU: draw-list command instead of immediate BufferUploader submission.
    drawList.fill(x0, y0, x1, y1, color);
  }

  protected fillGradient(drawList: GuiDrawList, x0: number, y0: number, x1: number, y1: number, topColor: number, bottomColor: number): void {
    GuiComponent.fillGradient(drawList, x0, y0, x1, y1, topColor, bottomColor);
  }

  public static fillGradient(drawList: GuiDrawList, x0: number, y0: number, x1: number, y1: number, topColor: number, bottomColor: number): void {
    // WebGPU: draw-list command instead of immediate BufferUploader submission.
    drawList.fillGradient(x0, y0, x1, y1, topColor, bottomColor);
  }

  public static drawCenteredString(drawList: GuiDrawList, font: Font, text: string, x: number, y: number, color: number): void {
    font.drawShadow(drawList, text, x - Math.floor(font.width(text) / 2), y, color);
  }

  public static drawString(drawList: GuiDrawList, font: Font, text: string, x: number, y: number, color: number): void {
    font.drawShadow(drawList, text, x, y, color);
  }

  protected blit(
    drawList: GuiDrawList,
    x: number,
    y: number,
    u: number,
    v: number,
    width: number,
    height: number,
    texture: GuiTextureId = "widgets",
    textureWidth = 256,
    textureHeight = 256,
    color = 0xffffffff,
  ): void {
    GuiComponent.blit(drawList, x, y, this.blitOffset, u, v, width, height, texture, textureWidth, textureHeight, color);
  }

  public static blit(
    drawList: GuiDrawList,
    x: number,
    y: number,
    _z: number,
    u: number,
    v: number,
    width: number,
    height: number,
    texture: GuiTextureId = "widgets",
    textureWidth = 256,
    textureHeight = 256,
    color = 0xffffffff,
  ): void {
    // WebGPU: textured quad draw command instead of mutating the currently bound GL texture.
    drawList.texturedQuad(
      texture,
      x,
      y,
      width,
      height,
      u / textureWidth,
      v / textureHeight,
      (u + width) / textureWidth,
      (v + height) / textureHeight,
      color,
    );
  }

  public getBlitOffset(): number {
    return this.blitOffset;
  }

  public setBlitOffset(blitOffset: number): void {
    this.blitOffset = blitOffset;
  }
}
