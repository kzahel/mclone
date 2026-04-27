export type TouchMoveButton = "forward" | "back";

export interface TouchControlTarget {
  readonly cssLeft: number;
  readonly cssTop: number;
  readonly cssWidth: number;
  readonly cssHeight: number;
}

export interface TouchMoveButtonRect {
  readonly kind: TouchMoveButton;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export function getTouchMoveButtonRects(target: TouchControlTarget): readonly TouchMoveButtonRect[] {
  if (target.cssWidth <= 0 || target.cssHeight <= 0) {
    return [];
  }

  const shortSide = Math.min(target.cssWidth, target.cssHeight);
  const size = clamp(Math.round(shortSide * 0.16), 48, 72);
  const gap = clamp(Math.round(size * 0.22), 10, 16);
  const rightMargin = clamp(Math.round(shortSide * 0.06), 20, 32);
  const bottomMargin = clamp(Math.round(shortSide * 0.24), 84, 120);
  const x = target.cssLeft + target.cssWidth - rightMargin - size;
  const backY = target.cssTop + target.cssHeight - bottomMargin - size;
  const forwardY = backY - gap - size;

  return [
    { kind: "forward", x, y: forwardY, width: size, height: size },
    { kind: "back", x, y: backY, width: size, height: size },
  ];
}

export function hitTouchMoveButton(clientX: number, clientY: number, target: TouchControlTarget): TouchMoveButton | null {
  for (const rect of getTouchMoveButtonRects(target)) {
    if (
      clientX >= rect.x
      && clientX <= rect.x + rect.width
      && clientY >= rect.y
      && clientY <= rect.y + rect.height
    ) {
      return rect.kind;
    }
  }
  return null;
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}
