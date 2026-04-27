import type { TouchControlsState, TouchJoystickState } from "../debug/debug-input";
import { getTouchMoveButtonRects, type TouchMoveButtonRect } from "../debug/touch-control-layout";
import type { GuiDrawList } from "./gui-draw-list";

export interface TouchJoystickHudTarget {
  readonly guiWidth: number;
  readonly guiHeight: number;
  readonly cssLeft: number;
  readonly cssTop: number;
  readonly cssWidth: number;
  readonly cssHeight: number;
}

const BASE_COLOR = 0x44000000;
const THUMB_COLOR = 0x88ffffff;
const BUTTON_COLOR = 0x55000000;
const BUTTON_ACTIVE_COLOR = 0x88ffffff;
const BUTTON_ICON_COLOR = 0xbbffffff;
const BUTTON_ACTIVE_ICON_COLOR = 0xee111111;
const MIN_BASE_RADIUS = 16;
const MIN_THUMB_RADIUS = 7;

export function renderTouchControlsHud(
  drawList: GuiDrawList,
  controls: TouchControlsState,
  target: TouchJoystickHudTarget,
): void {
  if (controls.visible) {
    renderMoveButtons(drawList, controls, target);
  }
  renderTouchJoystickHud(drawList, controls.joystick, target);
}

export function renderTouchJoystickHud(
  drawList: GuiDrawList,
  joystick: TouchJoystickState | null,
  target: TouchJoystickHudTarget,
): void {
  if (joystick === null || target.cssWidth <= 0 || target.cssHeight <= 0) {
    return;
  }

  const scaleX = target.guiWidth / target.cssWidth;
  const scaleY = target.guiHeight / target.cssHeight;
  const radiusScale = Math.min(scaleX, scaleY);
  const base = clientToGui(joystick.baseX, joystick.baseY, target);
  const thumb = clientToGui(joystick.thumbX, joystick.thumbY, target);
  const baseRadius = Math.max(MIN_BASE_RADIUS, joystick.maxDistance * radiusScale);
  const thumbRadius = Math.max(MIN_THUMB_RADIUS, baseRadius * 0.36);
  fillCircle(drawList, base.x, base.y, baseRadius, BASE_COLOR);
  fillCircle(drawList, thumb.x, thumb.y, thumbRadius, THUMB_COLOR);
}

function renderMoveButtons(drawList: GuiDrawList, controls: TouchControlsState, target: TouchJoystickHudTarget): void {
  if (target.cssWidth <= 0 || target.cssHeight <= 0) {
    return;
  }

  for (const rect of getTouchMoveButtonRects(target)) {
    renderMoveButton(
      drawList,
      rect,
      target,
      rect.kind === "forward" ? controls.moveForward : controls.moveBack,
    );
  }
}

function renderMoveButton(
  drawList: GuiDrawList,
  rect: TouchMoveButtonRect,
  target: TouchJoystickHudTarget,
  active: boolean,
): void {
  const topLeft = clientToGui(rect.x, rect.y, target);
  const bottomRight = clientToGui(rect.x + rect.width, rect.y + rect.height, target);
  const centerX = (topLeft.x + bottomRight.x) * 0.5;
  const centerY = (topLeft.y + bottomRight.y) * 0.5;
  const radius = Math.min(Math.abs(bottomRight.x - topLeft.x), Math.abs(bottomRight.y - topLeft.y)) * 0.5;
  fillCircle(drawList, centerX, centerY, radius, active ? BUTTON_ACTIVE_COLOR : BUTTON_COLOR);

  const iconColor = active ? BUTTON_ACTIVE_ICON_COLOR : BUTTON_ICON_COLOR;
  const iconWidth = radius * 0.9;
  const iconHeight = radius * 0.72;
  if (rect.kind === "forward") {
    fillTriangleUp(drawList, centerX, centerY - (iconHeight * 0.08), iconWidth, iconHeight, iconColor);
  } else {
    fillTriangleDown(drawList, centerX, centerY + (iconHeight * 0.08), iconWidth, iconHeight, iconColor);
  }
}

function clientToGui(clientX: number, clientY: number, target: TouchJoystickHudTarget): { readonly x: number; readonly y: number } {
  return {
    x: ((clientX - target.cssLeft) / target.cssWidth) * target.guiWidth,
    y: ((clientY - target.cssTop) / target.cssHeight) * target.guiHeight,
  };
}

function fillCircle(drawList: GuiDrawList, centerX: number, centerY: number, radius: number, color: number): void {
  const r = Math.max(1, Math.round(radius));
  const cx = Math.round(centerX);
  const cy = Math.round(centerY);
  for (let dy = -r; dy <= r; dy++) {
    const halfWidth = Math.floor(Math.sqrt((r * r) - (dy * dy)));
    drawList.fill(cx - halfWidth, cy + dy, cx + halfWidth + 1, cy + dy + 1, color);
  }
}

function fillTriangleUp(drawList: GuiDrawList, centerX: number, centerY: number, width: number, height: number, color: number): void {
  const h = Math.max(1, Math.round(height));
  const y0 = Math.round(centerY - (h * 0.5));
  for (let row = 0; row < h; row++) {
    const t = (row + 1) / h;
    const halfWidth = Math.max(1, Math.round((width * 0.5) * t));
    const y = y0 + row;
    drawList.fill(Math.round(centerX) - halfWidth, y, Math.round(centerX) + halfWidth + 1, y + 1, color);
  }
}

function fillTriangleDown(drawList: GuiDrawList, centerX: number, centerY: number, width: number, height: number, color: number): void {
  const h = Math.max(1, Math.round(height));
  const y0 = Math.round(centerY - (h * 0.5));
  for (let row = 0; row < h; row++) {
    const t = (h - row) / h;
    const halfWidth = Math.max(1, Math.round((width * 0.5) * t));
    const y = y0 + row;
    drawList.fill(Math.round(centerX) - halfWidth, y, Math.round(centerX) + halfWidth + 1, y + 1, color);
  }
}
