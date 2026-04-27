import type { GuiDrawList } from "../../renderer/gui/gui-draw-list";

export interface GlyphInfo {
  readonly codePoint: number;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly advance: number;
}

const ASCII_TEXTURE_SIZE = 128;
const ASCII_CELL_SIZE = 8;
const ASCII_COLUMNS = 16;
const FIRST_PRINTABLE_ASCII = 32;
const LAST_PRINTABLE_ASCII = 126;
const ASCII_GLYPH_ADVANCES = [
  4, 2, 4, 6, 6, 6, 6, 2, 4, 4, 4, 6, 2, 6, 2, 6,
  6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 2, 2, 5, 6, 5, 6,
  7, 6, 6, 6, 6, 6, 6, 6, 6, 4, 6, 6, 6, 6, 6, 6,
  6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 4, 6, 4, 6, 6,
  3, 6, 6, 6, 6, 6, 5, 6, 6, 2, 6, 5, 3, 6, 6, 6,
  6, 6, 6, 6, 4, 6, 6, 6, 6, 6, 6, 4, 2, 4, 7,
] as const;

export class Font {
  public readonly lineHeight = 9;

  public width(text: string): number {
    let width = 0;
    for (const char of text) {
      width += this.getGlyphInfo(char.codePointAt(0) ?? 0).advance;
    }
    return Math.ceil(width);
  }

  public draw(drawList: GuiDrawList, text: string, x: number, y: number, color: number): number {
    return this.drawInternal(drawList, text, x, y, color, false);
  }

  public drawShadow(drawList: GuiDrawList, text: string, x: number, y: number, color: number): number {
    return this.drawInternal(drawList, text, x, y, color, true);
  }

  public drawWordWrap(drawList: GuiDrawList, text: string, x: number, y: number, width: number, color: number): void {
    let line = "";
    let lineY = y;
    for (const word of text.split(" ")) {
      const next = line.length === 0 ? word : `${line} ${word}`;
      if (line.length > 0 && this.width(next) > width) {
        this.draw(drawList, line, x, lineY, color);
        line = word;
        lineY += this.lineHeight;
      } else {
        line = next;
      }
    }
    if (line.length > 0) {
      this.draw(drawList, line, x, lineY, color);
    }
  }

  public getGlyphInfo(codePoint: number): GlyphInfo {
    const resolvedCodePoint = codePoint >= FIRST_PRINTABLE_ASCII && codePoint <= LAST_PRINTABLE_ASCII ? codePoint : 63;
    return {
      codePoint: resolvedCodePoint,
      x: (resolvedCodePoint % ASCII_COLUMNS) * ASCII_CELL_SIZE,
      y: Math.floor(resolvedCodePoint / ASCII_COLUMNS) * ASCII_CELL_SIZE,
      width: ASCII_CELL_SIZE,
      height: ASCII_CELL_SIZE,
      advance: ASCII_GLYPH_ADVANCES[resolvedCodePoint - FIRST_PRINTABLE_ASCII] ?? ASCII_GLYPH_ADVANCES[63 - FIRST_PRINTABLE_ASCII]!,
    };
  }

  private drawInternal(drawList: GuiDrawList, text: string, x: number, y: number, color: number, dropShadow: boolean): number {
    if (text.length === 0) {
      return Math.trunc(x);
    }

    const adjustedColor = adjustColor(color);
    if (dropShadow) {
      this.drawGlyphRun(drawList, text, x + 1, y + 1, shadowColor(adjustedColor));
    }
    const endX = this.drawGlyphRun(drawList, text, x, y, adjustedColor);
    return Math.trunc(endX) + (dropShadow ? 1 : 0);
  }

  private drawGlyphRun(drawList: GuiDrawList, text: string, x: number, y: number, color: number): number {
    let cursorX = x;
    for (const char of text) {
      const glyph = this.getGlyphInfo(char.codePointAt(0) ?? 0);
      if (glyph.codePoint !== 32) {
        drawList.texturedQuad(
          "font",
          cursorX,
          y,
          glyph.width,
          glyph.height,
          glyph.x / ASCII_TEXTURE_SIZE,
          glyph.y / ASCII_TEXTURE_SIZE,
          (glyph.x + glyph.width) / ASCII_TEXTURE_SIZE,
          (glyph.y + glyph.height) / ASCII_TEXTURE_SIZE,
          color,
        );
      }
      cursorX += glyph.advance;
    }
    return cursorX;
  }
}

function adjustColor(color: number): number {
  return (color & 0xfc000000) === 0 ? (color | 0xff000000) >>> 0 : color >>> 0;
}

function shadowColor(color: number): number {
  const alpha = color & 0xff000000;
  const red = Math.trunc(((color >>> 16) & 0xff) * 0.25);
  const green = Math.trunc(((color >>> 8) & 0xff) * 0.25);
  const blue = Math.trunc((color & 0xff) * 0.25);
  return (alpha | (red << 16) | (green << 8) | blue) >>> 0;
}
