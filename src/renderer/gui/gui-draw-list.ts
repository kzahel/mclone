export type GuiTextureId = "font" | "widgets";

export interface GuiClipRect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export interface GuiSolidRectCommand {
  readonly type: "solid_rect";
  readonly x0: number;
  readonly y0: number;
  readonly x1: number;
  readonly y1: number;
  readonly color: number;
  readonly clip?: GuiClipRect;
}

export interface GuiGradientRectCommand {
  readonly type: "gradient_rect";
  readonly x0: number;
  readonly y0: number;
  readonly x1: number;
  readonly y1: number;
  readonly topColor: number;
  readonly bottomColor: number;
  readonly clip?: GuiClipRect;
}

export interface GuiTexturedQuadCommand {
  readonly type: "textured_quad";
  readonly texture: GuiTextureId;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly u0: number;
  readonly v0: number;
  readonly u1: number;
  readonly v1: number;
  readonly color: number;
  readonly clip?: GuiClipRect;
}

export type GuiDrawCommand =
  | GuiSolidRectCommand
  | GuiGradientRectCommand
  | GuiTexturedQuadCommand;

export class GuiDrawList {
  private readonly commands: GuiDrawCommand[] = [];
  private readonly clipStack: GuiClipRect[] = [];

  public clear(): void {
    this.commands.length = 0;
    this.clipStack.length = 0;
  }

  public getCommands(): readonly GuiDrawCommand[] {
    return this.commands;
  }

  public pushClip(x: number, y: number, width: number, height: number): void {
    const next = normalizeClip({ x, y, width, height });
    const current = this.currentClip();
    this.clipStack.push(current === undefined ? next : intersectClip(current, next));
  }

  public popClip(): void {
    this.clipStack.pop();
  }

  public fill(x0: number, y0: number, x1: number, y1: number, color: number): void {
    const rect = normalizeRect(x0, y0, x1, y1);
    if (rect.x0 === rect.x1 || rect.y0 === rect.y1) {
      return;
    }

    this.commands.push({
      type: "solid_rect",
      ...rect,
      color,
      clip: this.currentClip(),
    });
  }

  public fillGradient(x0: number, y0: number, x1: number, y1: number, topColor: number, bottomColor: number): void {
    const rect = normalizeRect(x0, y0, x1, y1);
    if (rect.x0 === rect.x1 || rect.y0 === rect.y1) {
      return;
    }

    this.commands.push({
      type: "gradient_rect",
      ...rect,
      topColor,
      bottomColor,
      clip: this.currentClip(),
    });
  }

  public texturedQuad(
    texture: GuiTextureId,
    x: number,
    y: number,
    width: number,
    height: number,
    u0: number,
    v0: number,
    u1: number,
    v1: number,
    color = 0xffffffff,
  ): void {
    if (width === 0 || height === 0) {
      return;
    }

    this.commands.push({
      type: "textured_quad",
      texture,
      x,
      y,
      width,
      height,
      u0,
      v0,
      u1,
      v1,
      color,
      clip: this.currentClip(),
    });
  }

  private currentClip(): GuiClipRect | undefined {
    return this.clipStack[this.clipStack.length - 1];
  }
}

function normalizeRect(x0: number, y0: number, x1: number, y1: number): Pick<GuiSolidRectCommand, "x0" | "y0" | "x1" | "y1"> {
  return {
    x0: Math.min(x0, x1),
    y0: Math.min(y0, y1),
    x1: Math.max(x0, x1),
    y1: Math.max(y0, y1),
  };
}

function normalizeClip(clip: GuiClipRect): GuiClipRect {
  const x0 = Math.min(clip.x, clip.x + clip.width);
  const y0 = Math.min(clip.y, clip.y + clip.height);
  const x1 = Math.max(clip.x, clip.x + clip.width);
  const y1 = Math.max(clip.y, clip.y + clip.height);
  return {
    x: x0,
    y: y0,
    width: x1 - x0,
    height: y1 - y0,
  };
}

function intersectClip(left: GuiClipRect, right: GuiClipRect): GuiClipRect {
  const x0 = Math.max(left.x, right.x);
  const y0 = Math.max(left.y, right.y);
  const x1 = Math.min(left.x + left.width, right.x + right.width);
  const y1 = Math.min(left.y + left.height, right.y + right.height);
  return {
    x: x0,
    y: y0,
    width: Math.max(0, x1 - x0),
    height: Math.max(0, y1 - y0),
  };
}
