import { isHexColor, type BlockSpec, type TintSpec } from "./dsl";
import {
  blendPixel,
  cloneImage,
  clamp01,
  clampByte,
  colorDistance,
  compositeImages,
  downsampleNearest,
  drawCheckerboard,
  drawGrid,
  drawRect,
  drawScaled,
  drawTiledScaled,
  hasTransparency,
  parseHexColor,
  random01,
  readPixel,
  shadeColor,
  solidImage,
  tintTexture,
  writePixel,
  type Rgba,
  type RgbaImage,
} from "./image";
import { drawPixelText, pad3, textPixelWidth } from "./text";
import type { AuthoringPreviewEntry, RenderedAuthoringPreview, RenderedTexture } from "./compositor";

type TilingMode = "xy" | "x" | "none";
type RotationPreviewMode = "none" | "y180" | "y90";
type LateralFaceName = "north" | "east" | "south" | "west";
type BlockFaceName = "top" | "bottom" | LateralFaceName;

const SEAM_ERROR_THRESHOLD = 0.04;

export interface ReviewSheetOptions {
  reference?: RgbaImage;
}

function effectiveTiling(texture: RenderedTexture, displayTexture: RgbaImage): TilingMode {
  if (texture.preview?.tiling) {
    return texture.preview.tiling;
  }
  const catalogTiling = texture.catalog?.tiling;
  if (catalogTiling === "xy" || catalogTiling === "x" || catalogTiling === "none") {
    return catalogTiling;
  }
  return hasTransparency(displayTexture) ? "none" : "xy";
}

function effectiveRotationPreview(texture: RenderedTexture): RotationPreviewMode {
  if (texture.preview?.rotation === false) {
    return "none";
  }
  if (texture.preview?.rotation === true) {
    return texture.catalog?.rotation === "y180-safe" ? "y180" : "y90";
  }
  if (texture.catalog?.rotation === "y90-safe") {
    return "y90";
  }
  if (texture.catalog?.rotation === "y180-safe") {
    return "y180";
  }
  return "none";
}

function drawTilingPreview(
  target: RgbaImage,
  texture: RgbaImage,
  targetX: number,
  targetY: number,
  tiling: TilingMode,
): void {
  if (tiling === "xy") {
    drawTiledScaled(target, texture, targetX, targetY, 3, 3, 3);
  } else if (tiling === "x") {
    drawTiledScaled(target, texture, targetX, targetY, 3, 1, 3);
  } else {
    drawScaled(target, texture, targetX, targetY, 8);
  }
}

function rotationPanelLabel(mode: RotationPreviewMode): string {
  return mode === "y180" ? "ROTATION 180 5X5" : "ROTATION 90 5X5";
}

function texturePanelLabel(prefix: string, tiling: TilingMode): string {
  if (tiling === "xy") {
    return `${prefix} TILED 3X3`;
  }
  if (tiling === "x") {
    return `${prefix} TILED 3X1`;
  }
  return `${prefix} TEXTURE`;
}

export function makeReviewSheet(texture: RenderedTexture, options: ReviewSheetOptions = {}): RgbaImage {
  const background: Rgba = [32, 34, 34, 255];
  const panel: Rgba = [52, 54, 54, 255];
  const structurePreview = texture.authoring.find((preview) => preview.role === "structure");
  const sheetWidth = 1040;
  const sheetHeight = structurePreview ? 864 : 672;
  const sheet = solidImage(sheetWidth, sheetHeight, background);
  const rawTexture = texture;
  const displayTexture = previewTexture(texture);
  const rawCheckerboard = texture.preview?.checkerboard ?? hasTransparency(rawTexture);
  const displayCheckerboard = texture.preview?.checkerboard ?? hasTransparency(displayTexture);
  const showCube = texture.preview?.cube ?? !hasTransparency(displayTexture);
  const rotationPreview = effectiveRotationPreview(texture);
  const showRotation = rotationPreview !== "none";
  const tiling = effectiveTiling(texture, displayTexture);

  drawRect(sheet, 16, 16, 264, 264, panel);
  if (rawCheckerboard) {
    drawCheckerboard(sheet, 20, 20, rawTexture.width * 8, rawTexture.height * 8, 8);
  }
  drawScaled(sheet, rawTexture, 20, 20, 8);
  drawGrid(sheet, 20, 20, rawTexture.width, rawTexture.height, 8, [86, 89, 88, 255]);

  drawRect(sheet, 304, 16, 296, 296, panel);
  if (displayCheckerboard && tiling !== "none") {
    drawCheckerboard(sheet, 308, 20, displayTexture.width * 3 * 3, displayTexture.height * 3 * 3, 12);
  }
  if (tiling === "xy") {
    drawTiledScaled(sheet, displayTexture, 308, 20, 3, 3, 3);
  } else if (tiling === "x") {
    if (displayCheckerboard) {
      drawCheckerboard(sheet, 308, 20, displayTexture.width * 3 * 3, displayTexture.height * 3, 12);
    }
    drawTiledScaled(sheet, displayTexture, 308, 20, 3, 1, 3);
  } else {
    if (displayCheckerboard) {
      drawCheckerboard(sheet, 308, 20, displayTexture.width * 3, displayTexture.height * 3, 12);
    }
    drawScaled(sheet, displayTexture, 308, 20, 3);
  }

  const downsampled = downsampleNearest(displayTexture, 16, 16);
  drawRect(sheet, 624, 16, 200, 200, panel);
  if (displayCheckerboard) {
    drawCheckerboard(sheet, 628, 20, downsampled.width * 12, downsampled.height * 12, 12);
  }
  drawScaled(sheet, downsampled, 628, 20, 12);
  drawGrid(sheet, 628, 20, downsampled.width, downsampled.height, 12, [86, 89, 88, 255]);

  drawRect(sheet, 624, 232, 200, 80, panel);
  drawMipStrip(sheet, displayTexture, 632, 240, displayCheckerboard);

  if (showCube) {
    drawRect(sheet, 840, 16, 184, 296, panel);
    drawIsometricCube(sheet, displayTexture, 868, 48);
  }

  if (showRotation) {
    drawRect(sheet, 16, 328, 328, 328, panel);
    drawRotatedTiledScaled(sheet, displayTexture, 20, 332, 5, 5, 2, `${texture.name}:rotation-preview`, rotationPreview);
  }

  if (tiling !== "none") {
    drawRect(sheet, 360, 328, 328, 328, panel);
    drawSeamDiagnostic(sheet, displayTexture, 360, 328, 328, 328, tiling, displayCheckerboard);
  }

  if (options.reference) {
    drawReferencePanel(sheet, options.reference, tiling, 704, 328, 320, 328);
  }

  if (structurePreview) {
    drawAuthoringPreviewPanel(sheet, structurePreview, 16, 672, 1008, 176);
  }

  drawPanelLabel(sheet, "SOURCE PIXELS", 16, 16, 264);
  drawPanelLabel(sheet, tilingLabel(tiling), 304, 16, 296);
  drawPanelLabel(sheet, "16X16 VIEW", 624, 16, 200);
  drawPanelLabel(sheet, "MIPS", 624, 232, 200, 1);
  if (showCube) {
    drawPanelLabel(sheet, "CUBE", 840, 16, 184);
  }
  if (showRotation) {
    drawPanelLabel(sheet, rotationPanelLabel(rotationPreview), 16, 328, 328);
  }
  if (tiling !== "none") {
    drawPanelLabel(sheet, tiling === "x" ? "SEAM 2X1" : "SEAM 2X2", 360, 328, 328);
    drawSeamSummary(sheet, displayTexture, tiling, 360, 328, 328, 328);
  }

  return sheet;
}

export function makeBlockReviewSheet(
  blockName: string,
  block: BlockSpec,
  texturesByName: Map<string, RenderedTexture>,
): RgbaImage {
  const background: Rgba = [32, 34, 34, 255];
  const panel: Rgba = [52, 54, 54, 255];
  const sheet = solidImage(1040, 672, background);
  const tintColors = tintColorsForBlock(block, texturesByName);
  const primaryTint = tintColors[0]!;

  drawRect(sheet, 16, 16, 300, 300, panel);
  if (hasDirectionalFaces(block)) {
    drawDirectionalCubePreview(sheet, block, texturesByName, primaryTint, 16, 16);
  } else {
    drawIsometricCubeFaces(sheet, cubeFacesForYaw(block, texturesByName, primaryTint, "north"), 82, 58);
  }

  const top = textureForFace(block, texturesByName, "top");
  const topDisplay = applyRoleTint(top, primaryTint);
  const topRotation = effectiveRotationPreview(top);
  const topTiling = effectiveTiling(top, topDisplay);
  drawRect(sheet, 340, 16, 328, 328, panel);
  if (topRotation !== "none") {
    drawRotatedTiledScaled(sheet, topDisplay, 344, 20, 5, 5, 2, `${blockName}:top-rotation`, topRotation);
  } else {
    drawTilingPreview(sheet, topDisplay, 344, 20, topTiling);
  }

  const side = compositeSideTexture(block, texturesByName, primaryTint);
  const sideTiling = effectiveTiling(textureForFace(block, texturesByName, "side"), side);
  drawRect(sheet, 692, 16, 328, 328, panel);
  if (hasDirectionalFaces(block)) {
    drawDirectionalFacePanel(sheet, block, texturesByName, primaryTint, 692, 16);
  } else {
    drawTilingPreview(sheet, side, 696, 20, sideTiling);
  }

  drawRect(sheet, 16, 360, 1008, 280, panel);
  drawTerrainPatch(sheet, block, texturesByName, primaryTint, 32, 372);
  drawIsometricCubeFaces(sheet, cubeFacesForYaw(block, texturesByName, primaryTint, "north"), 760, 410);
  for (const [index, tint] of tintColors.slice(0, 4).entries()) {
    drawTintSwatch(sheet, 890, 412 + index * 34, tint);
  }

  drawPanelLabel(sheet, "BLOCK PREVIEW", 16, 16, 300);
  drawPanelLabel(sheet, topRotation !== "none" ? `TOP ${rotationPanelLabel(topRotation)}` : texturePanelLabel("TOP", topTiling), 340, 16, 328);
  drawPanelLabel(sheet, hasDirectionalFaces(block) ? "DIRECTIONAL FACES" : texturePanelLabel("SIDE", sideTiling), 692, 16, 328);
  drawPanelLabel(sheet, "TERRAIN PATCH", 16, 360, 1008);
  drawPanelLabel(sheet, "FINAL CUBE", 744, 386, 160, 1);
  drawPanelLabel(sheet, "TINTS", 880, 386, 100, 1);

  return sheet;
}

export function makeBlockSideReviewSheet(
  blockName: string,
  block: BlockSpec,
  texturesByName: Map<string, RenderedTexture>,
): RgbaImage | undefined {
  if (!block.faces.side || !block.faces.overlay) {
    return undefined;
  }

  const background: Rgba = [32, 34, 34, 255];
  const panel: Rgba = [52, 54, 54, 255];
  const sheet = solidImage(1040, 432, background);
  const tintColors = tintColorsForBlock(block, texturesByName);
  const primaryTint = tintColors[0]!;
  const side = textureForFace(block, texturesByName, "side");
  const overlay = texturesByName.get(block.faces.overlay);
  if (!overlay) {
    throw new Error(`Block '${blockName}' overlay references missing rendered texture '${block.faces.overlay}'`);
  }
  const composedSide = compositeImages(cloneImage(side), applyRoleTint(overlay, primaryTint));
  const top = applyRoleTint(textureForFace(block, texturesByName, "top"), primaryTint);

  drawRect(sheet, 16, 16, 264, 264, panel);
  drawScaled(sheet, side, 20, 20, 8);
  drawGrid(sheet, 20, 20, side.width, side.height, 8, [86, 89, 88, 255]);

  drawRect(sheet, 304, 16, 264, 264, panel);
  drawCheckerboard(sheet, 308, 20, overlay.width * 8, overlay.height * 8, 8);
  drawScaled(sheet, overlay, 308, 20, 8);
  drawGrid(sheet, 308, 20, overlay.width, overlay.height, 8, [86, 89, 88, 255]);

  drawRect(sheet, 592, 16, 432, 128, panel);
  drawTiledScaled(sheet, composedSide, 608, 32, 4, 1, 3);

  drawRect(sheet, 592, 160, 432, 248, panel);
  drawScaled(sheet, top, 608, 176, 3);
  drawGrid(sheet, 608, 176, top.width, top.height, 3, [86, 89, 88, 255]);
  drawScaled(sheet, composedSide, 728, 176, 3);
  drawGrid(sheet, 728, 176, composedSide.width, composedSide.height, 3, [86, 89, 88, 255]);
  drawIsometricCubeFaces(sheet, cubeFacesForYaw(block, texturesByName, primaryTint, "north"), 864, 216);
  for (const [index, tint] of tintColors.slice(0, 4).entries()) {
    drawTintSwatch(sheet, 608, 296 + index * 26, tint);
  }

  drawPanelLabel(sheet, "SIDE BASE PIXELS", 16, 16, 264);
  drawPanelLabel(sheet, "OVERLAY PIXELS", 304, 16, 264);
  drawPanelLabel(sheet, "FINAL SIDE 4X1", 592, 16, 432);
  drawPanelLabel(sheet, "BLOCK CONTEXT", 592, 160, 432);

  return sheet;
}

function drawRotatedTiledScaled(
  target: RgbaImage,
  source: RgbaImage,
  targetX: number,
  targetY: number,
  tilesX: number,
  tilesY: number,
  scale: number,
  seed: string,
  mode: RotationPreviewMode,
): void {
  for (let tileY = 0; tileY < tilesY; tileY += 1) {
    for (let tileX = 0; tileX < tilesX; tileX += 1) {
      const rotation = rotatedTileStep(seed, tileX, tileY, mode);
      drawRotatedScaled(
        target,
        source,
        targetX + tileX * source.width * scale,
        targetY + tileY * source.height * scale,
        scale,
        rotation,
      );
    }
  }
}

function rotatedTileStep(seed: string, tileX: number, tileY: number, mode: RotationPreviewMode): number {
  if (mode === "y180") {
    return random01(seed, tileX, tileY) < 0.5 ? 0 : 2;
  }
  return Math.floor(random01(seed, tileX, tileY) * 4) % 4;
}

function drawRotatedScaled(
  target: RgbaImage,
  source: RgbaImage,
  targetX: number,
  targetY: number,
  scale: number,
  rotation: number,
): void {
  for (let y = 0; y < source.height; y += 1) {
    for (let x = 0; x < source.width; x += 1) {
      const [sourceX, sourceY] = rotatedSourceCoordinate(source.width, source.height, x, y, rotation);
      const color = readPixel(source, sourceX, sourceY);
      drawRect(target, targetX + x * scale, targetY + y * scale, scale, scale, color);
    }
  }
}

function rotatedSourceCoordinate(width: number, height: number, x: number, y: number, rotation: number): [number, number] {
  if (rotation === 1) {
    return [y, height - 1 - x];
  }
  if (rotation === 2) {
    return [width - 1 - x, height - 1 - y];
  }
  if (rotation === 3) {
    return [width - 1 - y, x];
  }
  return [x, y];
}

function drawMipStrip(target: RgbaImage, source: RgbaImage, targetX: number, targetY: number, checkerboard: boolean): void {
  const levels = [
    { size: 16, scale: 3 },
    { size: 8, scale: 4 },
    { size: 4, scale: 6 },
    { size: 2, scale: 10 },
    { size: 1, scale: 16 },
  ];
  let x = targetX;
  for (const level of levels) {
    const image = downsampleNearest(source, level.size, level.size);
    if (checkerboard) {
      drawCheckerboard(target, x, targetY, level.size * level.scale, level.size * level.scale, level.scale);
    }
    drawScaled(target, image, x, targetY, level.scale);
    x += level.size * level.scale + 12;
  }
}

function drawCompactMipStrip(
  target: RgbaImage,
  source: RgbaImage,
  targetX: number,
  targetY: number,
  checkerboard: boolean,
): void {
  const levels = [
    { size: 16, scale: 2 },
    { size: 8, scale: 3 },
    { size: 4, scale: 4 },
    { size: 2, scale: 5 },
    { size: 1, scale: 8 },
  ];
  let x = targetX;
  for (const level of levels) {
    const image = downsampleNearest(source, level.size, level.size);
    if (checkerboard) {
      drawCheckerboard(target, x, targetY, level.size * level.scale, level.size * level.scale, level.scale);
    }
    drawScaled(target, image, x, targetY, level.scale);
    x += level.size * level.scale + 10;
  }
}

function drawSeamDiagnostic(
  target: RgbaImage,
  source: RgbaImage,
  panelX: number,
  panelY: number,
  panelWidth: number,
  panelHeight: number,
  tiling: TilingMode,
  checkerboard: boolean,
): void {
  const tilesX = 2;
  const tilesY = tiling === "xy" ? 2 : 1;
  const padding = 28;
  const scale = Math.max(
    1,
    Math.floor(
      Math.min(
        (panelWidth - padding * 2) / (source.width * tilesX),
        (panelHeight - padding * 2) / (source.height * tilesY),
      ),
    ),
  );
  const previewWidth = source.width * tilesX * scale;
  const previewHeight = source.height * tilesY * scale;
  const previewX = panelX + Math.floor((panelWidth - previewWidth) / 2);
  const previewY = panelY + Math.floor((panelHeight - previewHeight) / 2);

  if (checkerboard) {
    drawCheckerboard(target, previewX, previewY, previewWidth, previewHeight, Math.max(4, scale * 2));
  }
  drawTiledScaled(target, source, previewX, previewY, tilesX, tilesY, scale);

  const seamWidth = Math.max(2, Math.floor(scale / 2));
  const verticalSeamX = previewX + source.width * scale - Math.floor(seamWidth / 2);
  drawVerticalSeamError(target, source, verticalSeamX, previewY, tilesY, scale, seamWidth);

  if (tiling === "xy") {
    const horizontalSeamY = previewY + source.height * scale - Math.floor(seamWidth / 2);
    drawHorizontalSeamError(target, source, previewX, horizontalSeamY, tilesX, scale, seamWidth);
  }
}

interface SeamStats {
  score: number;
  xMarks: number;
  yMarks: number;
  meanDiff: number;
  maxDiff: number;
}

function seamStats(source: RgbaImage, tiling: TilingMode): SeamStats {
  let totalDiff = 0;
  let maxDiff = 0;
  let samples = 0;
  let xMarks = 0;
  let yMarks = 0;

  for (let y = 0; y < source.height; y += 1) {
    const diff = colorDistance(readPixel(source, source.width - 1, y), readPixel(source, 0, y));
    totalDiff += diff;
    maxDiff = Math.max(maxDiff, diff);
    samples += 1;
    if (diff >= SEAM_ERROR_THRESHOLD) {
      xMarks += 1;
    }
  }

  if (tiling === "xy") {
    for (let x = 0; x < source.width; x += 1) {
      const diff = colorDistance(readPixel(source, x, source.height - 1), readPixel(source, x, 0));
      totalDiff += diff;
      maxDiff = Math.max(maxDiff, diff);
      samples += 1;
      if (diff >= SEAM_ERROR_THRESHOLD) {
        yMarks += 1;
      }
    }
  }

  const meanDiff = samples > 0 ? totalDiff / samples : 0;
  const score = Math.round(100 * clamp01(1 - Math.max(meanDiff / 0.08, maxDiff / 0.32)));
  return { score, xMarks, yMarks, meanDiff, maxDiff };
}

function drawSeamSummary(
  target: RgbaImage,
  source: RgbaImage,
  tiling: TilingMode,
  panelX: number,
  panelY: number,
  panelWidth: number,
  panelHeight: number,
): void {
  const stats = seamStats(source, tiling);
  const y = panelY + panelHeight - 42;
  drawRect(target, panelX + 2, y, panelWidth - 4, 40, [42, 44, 44, 235]);
  drawPixelText(target, `CONTINUITY ${pad3(stats.score)} 100 BEST`, panelX + 10, y + 7, 1, [214, 218, 210, 255]);
  drawRect(target, panelX + 10, y + 24, 9, 9, [255, 56, 24, 255]);
  drawPixelText(target, `RED X ${pad3(stats.xMarks)}`, panelX + 24, y + 25, 1, [214, 218, 210, 255]);
  if (tiling === "xy") {
    drawRect(target, panelX + 120, y + 24, 9, 9, [24, 148, 255, 255]);
    drawPixelText(target, `BLUE Y ${pad3(stats.yMarks)}`, panelX + 134, y + 25, 1, [214, 218, 210, 255]);
  }
}

function tilingLabel(tiling: TilingMode): string {
  if (tiling === "xy") {
    return "TILED 3X3";
  }
  if (tiling === "x") {
    return "TILED 3X1";
  }
  return "SINGLE 1X";
}

function drawPanelLabel(
  target: RgbaImage,
  text: string,
  panelX: number,
  panelY: number,
  panelWidth: number,
  preferredScale = 2,
): void {
  const scale = textPixelWidth(text, preferredScale) > panelWidth - 20 ? 1 : preferredScale;
  const labelHeight = 7 * scale + 8;
  drawRect(target, panelX + 2, panelY + 2, panelWidth - 4, labelHeight, [42, 44, 44, 235]);
  drawPixelText(target, text, panelX + 10, panelY + 6, scale, [214, 218, 210, 255]);
}

function drawReferencePanel(
  target: RgbaImage,
  reference: RgbaImage,
  tiling: TilingMode,
  panelX: number,
  panelY: number,
  panelWidth: number,
  panelHeight: number,
): void {
  const panel: Rgba = [52, 54, 54, 255];
  const border: Rgba = [116, 119, 116, 255];
  drawRect(target, panelX, panelY, panelWidth, panelHeight, panel);
  drawRect(target, panelX, panelY, panelWidth, 2, border);
  drawRect(target, panelX, panelY + panelHeight - 2, panelWidth, 2, border);
  drawRect(target, panelX, panelY, 2, panelHeight, border);
  drawRect(target, panelX + panelWidth - 2, panelY, 2, panelHeight, border);
  drawRect(target, panelX + 2, panelY + 2, panelWidth - 4, 22, [42, 44, 44, 255]);
  drawPixelText(target, "MINECRAFT REFERENCE", panelX + 18, panelY + 8, 2, [214, 218, 210, 255]);

  const checkerboard = hasTransparency(reference);
  const enlargedScale = Math.max(1, Math.floor(Math.min(128 / reference.width, 128 / reference.height)));
  const enlargedWidth = reference.width * enlargedScale;
  const enlargedHeight = reference.height * enlargedScale;
  const enlargedX = panelX + 18;
  const enlargedY = panelY + 42;
  drawPixelText(target, "PIXELS", enlargedX, panelY + 30, 1, [214, 218, 210, 255]);
  if (checkerboard) {
    drawCheckerboard(target, enlargedX, enlargedY, enlargedWidth, enlargedHeight, Math.max(4, enlargedScale));
  }
  drawScaled(target, reference, enlargedX, enlargedY, enlargedScale);
  drawGrid(target, enlargedX, enlargedY, reference.width, reference.height, enlargedScale, [86, 89, 88, 255]);

  const repeatTilesX = tiling === "none" ? 1 : 3;
  const repeatTilesY = tiling === "xy" ? 3 : 1;
  const repeatScale = Math.max(
    1,
    Math.floor(Math.min(144 / (reference.width * repeatTilesX), 144 / (reference.height * repeatTilesY))),
  );
  const repeatWidth = reference.width * repeatTilesX * repeatScale;
  const repeatHeight = reference.height * repeatTilesY * repeatScale;
  const repeatX = panelX + panelWidth - repeatWidth - 18;
  const repeatY = panelY + 42;
  drawPixelText(target, tilingLabel(tiling), repeatX, panelY + 30, 1, [214, 218, 210, 255]);
  if (checkerboard) {
    drawCheckerboard(target, repeatX, repeatY, repeatWidth, repeatHeight, Math.max(4, repeatScale * 2));
  }
  if (tiling === "none") {
    drawScaled(target, reference, repeatX, repeatY, repeatScale);
  } else {
    drawTiledScaled(target, reference, repeatX, repeatY, repeatTilesX, repeatTilesY, repeatScale);
  }

  const lowerY = panelY + 42 + Math.max(enlargedHeight, repeatHeight) + 20;
  if (lowerY + 100 < panelY + panelHeight) {
    drawPixelText(target, "MIPS", panelX + 18, lowerY - 12, 1, [214, 218, 210, 255]);
    drawCompactMipStrip(target, reference, panelX + 18, lowerY, checkerboard);

    if (!checkerboard) {
      const cubeX = panelX + panelWidth - 108;
      drawPixelText(target, "CUBE", cubeX, lowerY - 12, 1, [214, 218, 210, 255]);
      drawSmallIsometricCube(target, reference, cubeX, lowerY);
    }
  }
}

function drawAuthoringPreviewPanel(
  target: RgbaImage,
  preview: RenderedAuthoringPreview,
  panelX: number,
  panelY: number,
  panelWidth: number,
  panelHeight: number,
): void {
  const panel: Rgba = [52, 54, 54, 255];
  const labelColor: Rgba = [214, 218, 210, 255];
  const border: Rgba = [86, 89, 88, 255];
  drawRect(target, panelX, panelY, panelWidth, panelHeight, panel);
  drawPanelLabel(target, preview.label, panelX, panelY, panelWidth);

  const contentY = panelY + 44;
  const checkerboard = hasTransparency(preview);
  const sourceScale = Math.max(1, Math.floor(Math.min(128 / preview.width, 128 / preview.height)));
  const sourceX = panelX + 18;
  drawPixelText(target, `SOURCE ${preview.width}X${preview.height}`, sourceX, panelY + 32, 1, labelColor);
  if (checkerboard) {
    drawCheckerboard(target, sourceX, contentY, preview.width * sourceScale, preview.height * sourceScale, sourceScale);
  }
  drawScaled(target, preview, sourceX, contentY, sourceScale);
  drawGrid(target, sourceX, contentY, preview.width, preview.height, sourceScale, border);

  const tiledScale = Math.max(1, Math.floor(Math.min(112 / (preview.width * 3), 112 / (preview.height * 3))));
  const tiledX = panelX + 180;
  const tiledWidth = preview.width * 3 * tiledScale;
  const tiledHeight = preview.height * 3 * tiledScale;
  drawPixelText(target, "MASK TILED 3X3", tiledX, panelY + 32, 1, labelColor);
  if (checkerboard) {
    drawCheckerboard(target, tiledX, contentY, tiledWidth, tiledHeight, Math.max(4, tiledScale * 2));
  }
  drawTiledScaled(target, preview, tiledX, contentY, 3, 3, tiledScale);

  drawPixelText(target, "SYMBOLS", panelX + 340, panelY + 32, 1, labelColor);
  drawAuthoringLegend(target, preview.entries, panelX + 340, contentY);
}

function drawAuthoringLegend(
  target: RgbaImage,
  entries: AuthoringPreviewEntry[],
  x: number,
  y: number,
): void {
  const sorted = [...entries].sort((a, b) => a.symbol.localeCompare(b.symbol));
  const rowHeight = 18;
  const columnWidth = 164;
  const rowsPerColumn = 7;
  for (const [index, entry] of sorted.entries()) {
    const column = Math.floor(index / rowsPerColumn);
    const row = index % rowsPerColumn;
    const entryX = x + column * columnWidth;
    const entryY = y + row * rowHeight;
    drawRect(target, entryX, entryY, 14, 14, [24, 26, 26, 255]);
    drawRect(target, entryX + 2, entryY + 2, 10, 10, entry.color);
    drawPixelText(target, authoringLegendText(entry), entryX + 20, entryY + 3, 1, [214, 218, 210, 255]);
  }
}

function authoringLegendText(entry: AuthoringPreviewEntry): string {
  const name = isHexColor(entry.name) ? "COLOR" : entry.name.replace(/_/g, " ");
  return `${entry.symbol} ${name}`;
}

function drawVerticalSeamError(
  target: RgbaImage,
  source: RgbaImage,
  targetX: number,
  targetY: number,
  tilesY: number,
  scale: number,
  seamWidth: number,
): void {
  for (let y = 0; y < source.height; y += 1) {
    const color = seamErrorColor(readPixel(source, source.width - 1, y), readPixel(source, 0, y), "x");
    if (color[3] === 0) {
      continue;
    }
    for (let tileY = 0; tileY < tilesY; tileY += 1) {
      drawRect(target, targetX, targetY + (tileY * source.height + y) * scale, seamWidth, scale, color);
    }
  }
}

function drawHorizontalSeamError(
  target: RgbaImage,
  source: RgbaImage,
  targetX: number,
  targetY: number,
  tilesX: number,
  scale: number,
  seamWidth: number,
): void {
  for (let x = 0; x < source.width; x += 1) {
    const color = seamErrorColor(readPixel(source, x, source.height - 1), readPixel(source, x, 0), "y");
    if (color[3] === 0) {
      continue;
    }
    for (let tileX = 0; tileX < tilesX; tileX += 1) {
      drawRect(target, targetX + (tileX * source.width + x) * scale, targetY, scale, seamWidth, color);
    }
  }
}

function seamErrorColor(a: Rgba, b: Rgba, axis: "x" | "y"): Rgba {
  const diff = colorDistance(a, b);
  if (diff < SEAM_ERROR_THRESHOLD) {
    return [0, 0, 0, 0];
  }
  const alpha = clampByte(Math.min(0.9, diff * 1.8) * 255);
  return axis === "x" ? [255, 56, 24, alpha] : [24, 148, 255, alpha];
}

function drawIsometricCube(target: RgbaImage, texture: RgbaImage, targetX: number, targetY: number): void {
  drawIsometricCubeFaces(
    target,
    {
      top: texture,
      left: texture,
      right: texture,
    },
    targetX,
    targetY,
  );
}

function drawSmallIsometricCube(target: RgbaImage, texture: RgbaImage, targetX: number, targetY: number): void {
  drawIsometricCubeFacesSized(
    target,
    {
      top: texture,
      left: texture,
      right: texture,
    },
    targetX,
    targetY,
    42,
    21,
    56,
  );
}

interface CubeFaceImages {
  top: RgbaImage;
  left: RgbaImage;
  right: RgbaImage;
}

function drawIsometricCubeFaces(target: RgbaImage, faces: CubeFaceImages, targetX: number, targetY: number): void {
  drawIsometricCubeFacesSized(target, faces, targetX, targetY, 64, 32, 84);
}

function drawIsometricCubeFacesSized(
  target: RgbaImage,
  faces: CubeFaceImages,
  targetX: number,
  targetY: number,
  halfWidth: number,
  halfDepth: number,
  height: number,
): void {
  const originX = targetX + halfWidth;
  const originY = targetY + height;

  drawCubeFace(target, faces.left, 0.68, (u, v) => projectIso(originX, originY, halfWidth, halfDepth, height, u, 1 - v, 1));
  drawCubeFace(target, faces.right, 0.78, (u, v) => projectIso(originX, originY, halfWidth, halfDepth, height, 1, 1 - v, u));
  drawCubeFace(target, faces.top, 1.08, (u, v) => projectIso(originX, originY, halfWidth, halfDepth, height, u, 1, v));
  drawCubeOutline(target, [
    projectIso(originX, originY, halfWidth, halfDepth, height, 0, 1, 0),
    projectIso(originX, originY, halfWidth, halfDepth, height, 1, 1, 0),
    projectIso(originX, originY, halfWidth, halfDepth, height, 1, 1, 1),
    projectIso(originX, originY, halfWidth, halfDepth, height, 0, 1, 1),
    projectIso(originX, originY, halfWidth, halfDepth, height, 0, 0, 1),
    projectIso(originX, originY, halfWidth, halfDepth, height, 1, 0, 1),
    projectIso(originX, originY, halfWidth, halfDepth, height, 1, 0, 0),
  ]);
}

function cubeFacesForYaw(
  block: BlockSpec,
  texturesByName: Map<string, RenderedTexture>,
  tint: Rgba,
  yaw: LateralFaceName,
): CubeFaceImages {
  return {
    top: applyRoleTint(textureForBlockFace(block, texturesByName, "top"), tint),
    left: compositeFaceTexture(block, texturesByName, tint, yaw),
    right: compositeFaceTexture(block, texturesByName, tint, rightFaceForYaw(yaw)),
  };
}

function drawDirectionalCubePreview(
  target: RgbaImage,
  block: BlockSpec,
  texturesByName: Map<string, RenderedTexture>,
  tint: Rgba,
  panelX: number,
  panelY: number,
): void {
  const yaws: LateralFaceName[] = ["north", "east", "south", "west"];
  for (const [index, yaw] of yaws.entries()) {
    const column = index % 2;
    const row = Math.floor(index / 2);
    const x = panelX + 42 + column * 120;
    const y = panelY + 42 + row * 120;
    drawIsometricCubeFacesSized(target, cubeFacesForYaw(block, texturesByName, tint, yaw), x, y, 42, 21, 56);
    drawPixelText(target, `${yaw.toUpperCase()}+${rightFaceForYaw(yaw).toUpperCase()}`, x - 4, y + 104, 1, [214, 218, 210, 255]);
  }
}

function drawDirectionalFacePanel(
  target: RgbaImage,
  block: BlockSpec,
  texturesByName: Map<string, RenderedTexture>,
  tint: Rgba,
  panelX: number,
  panelY: number,
): void {
  const entries: { label: string; face: BlockFaceName }[] = [
    { label: "TOP", face: "top" },
    { label: "BOTTOM", face: "bottom" },
    { label: "NORTH", face: "north" },
    { label: "EAST", face: "east" },
    { label: "SOUTH", face: "south" },
    { label: "WEST", face: "west" },
  ];
  for (const [index, entry] of entries.entries()) {
    const column = index % 3;
    const row = Math.floor(index / 3);
    const x = panelX + 20 + column * 102;
    const y = panelY + 54 + row * 122;
    const image =
      entry.face === "top" || entry.face === "bottom"
        ? applyRoleTint(textureForBlockFace(block, texturesByName, entry.face), tint)
        : compositeFaceTexture(block, texturesByName, tint, entry.face);
    const scale = Math.max(1, Math.min(2, Math.floor(72 / Math.max(image.width, image.height))));
    drawPixelText(target, entry.label, x, y - 14, 1, [214, 218, 210, 255]);
    drawScaled(target, image, x, y, scale);
    drawGrid(target, x, y, image.width, image.height, scale, [86, 89, 88, 255]);
  }
}

function rightFaceForYaw(yaw: LateralFaceName): LateralFaceName {
  if (yaw === "north") {
    return "east";
  }
  if (yaw === "east") {
    return "south";
  }
  if (yaw === "south") {
    return "west";
  }
  return "north";
}

function hasDirectionalFaces(block: BlockSpec): boolean {
  return Boolean(block.faces.north || block.faces.east || block.faces.south || block.faces.west);
}

function tintColorsForBlock(block: BlockSpec, texturesByName: Map<string, RenderedTexture>): Rgba[] {
  const tint = tintSpecForBlock(block, texturesByName);
  if (!tint) {
    return [[255, 255, 255, 255]];
  }
  return [tint.normal, ...(tint.alternates ?? [])].map(parseHexColor);
}

function tintSpecForBlock(block: BlockSpec, texturesByName: Map<string, RenderedTexture>): TintSpec | undefined {
  const textureNames = [
    block.faces.top,
    block.faces.overlay,
    block.faces.side,
    block.faces.north,
    block.faces.east,
    block.faces.south,
    block.faces.west,
    block.faces.bottom,
    block.faces.all,
  ];
  for (const textureName of textureNames) {
    if (!textureName) {
      continue;
    }
    const texture = texturesByName.get(textureName);
    if (texture?.tint) {
      return texture.tint;
    }
  }
  return undefined;
}

function textureForFace(block: BlockSpec, texturesByName: Map<string, RenderedTexture>, face: "top" | "bottom" | "side"): RenderedTexture {
  const textureName = face === "side" ? lateralTextureName(block, "north") : textureNameForBlockFace(block, face);
  if (!textureName) {
    throw new Error(`Block face '${face}' has no texture`);
  }
  const texture = texturesByName.get(textureName);
  if (!texture) {
    throw new Error(`Block face '${face}' references missing rendered texture '${textureName}'`);
  }
  return texture;
}

function compositeSideTexture(block: BlockSpec, texturesByName: Map<string, RenderedTexture>, tint: Rgba): RgbaImage {
  return compositeFaceTexture(block, texturesByName, tint, "north");
}

function compositeFaceTexture(
  block: BlockSpec,
  texturesByName: Map<string, RenderedTexture>,
  tint: Rgba,
  face: LateralFaceName,
): RgbaImage {
  const side = cloneImage(textureForBlockFace(block, texturesByName, face));
  const overlayName = block.faces.overlay;
  if (!overlayName) {
    return side;
  }
  const overlay = texturesByName.get(overlayName);
  if (!overlay) {
    throw new Error(`Block overlay references missing rendered texture '${overlayName}'`);
  }
  return compositeImages(side, applyRoleTint(overlay, tint));
}

function textureForBlockFace(block: BlockSpec, texturesByName: Map<string, RenderedTexture>, face: BlockFaceName): RenderedTexture {
  const textureName = textureNameForBlockFace(block, face);
  if (!textureName) {
    throw new Error(`Block face '${face}' has no texture`);
  }
  const texture = texturesByName.get(textureName);
  if (!texture) {
    throw new Error(`Block face '${face}' references missing rendered texture '${textureName}'`);
  }
  return texture;
}

function textureNameForBlockFace(block: BlockSpec, face: BlockFaceName): string | undefined {
  if (face === "top") {
    return block.faces.top ?? block.faces.all ?? block.faces.side;
  }
  if (face === "bottom") {
    return block.faces.bottom ?? block.faces.all ?? block.faces.top ?? block.faces.side;
  }
  return lateralTextureName(block, face);
}

function lateralTextureName(block: BlockSpec, face: LateralFaceName): string | undefined {
  return block.faces[face] ?? block.faces.side ?? block.faces.all;
}

function drawTerrainPatch(
  target: RgbaImage,
  block: BlockSpec,
  texturesByName: Map<string, RenderedTexture>,
  tint: Rgba,
  targetX: number,
  targetY: number,
): void {
  const top = applyRoleTint(textureForFace(block, texturesByName, "top"), tint);
  const dirt = textureForFace(block, texturesByName, "bottom");
  const tileScale = 2;
  const tileSize = top.width * tileScale;
  for (let row = 0; row < 4; row += 1) {
    for (let column = 0; column < 10; column += 1) {
      const useDirt = random01("terrain-patch-dirt", column, row) < 0.12;
      const image = useDirt ? dirt : top;
      const rotation = useDirt ? 0 : Math.floor(random01("terrain-patch-grass-rotation", column, row) * 4) % 4;
      drawRotatedScaled(target, image, targetX + column * tileSize, targetY + row * tileSize, tileScale, rotation);
    }
  }
}

function drawCubeFace(
  target: RgbaImage,
  texture: RgbaImage,
  shade: number,
  map: (u: number, v: number) => Point,
): void {
  for (let y = 0; y < texture.height; y += 1) {
    for (let x = 0; x < texture.width; x += 1) {
      const u0 = x / texture.width;
      const v0 = y / texture.height;
      const u1 = (x + 1) / texture.width;
      const v1 = (y + 1) / texture.height;
      fillPolygon(
        target,
        [map(u0, v0), map(u1, v0), map(u1, v1), map(u0, v1)],
        shadeColor(readPixel(texture, x, y), shade),
      );
    }
  }
}

function projectIso(
  originX: number,
  originY: number,
  halfWidth: number,
  halfDepth: number,
  height: number,
  x: number,
  y: number,
  z: number,
): Point {
  return {
    x: originX + (x - z) * halfWidth,
    y: originY + (x + z) * halfDepth - y * height,
  };
}

function drawCubeOutline(target: RgbaImage, points: Point[]): void {
  const outline: Rgba = [24, 22, 20, 255];
  const [topFront, topRight, topBack, topLeft, bottomLeft, bottomBack, bottomRight] = points;
  drawLine(target, topFront!, topRight!, outline);
  drawLine(target, topRight!, topBack!, outline);
  drawLine(target, topBack!, topLeft!, outline);
  drawLine(target, topLeft!, topFront!, outline);
  drawLine(target, topLeft!, bottomLeft!, outline);
  drawLine(target, topBack!, bottomBack!, outline);
  drawLine(target, topRight!, bottomRight!, outline);
  drawLine(target, bottomLeft!, bottomBack!, outline);
  drawLine(target, bottomBack!, bottomRight!, outline);
}

interface Point {
  x: number;
  y: number;
}

function fillPolygon(target: RgbaImage, polygon: Point[], color: Rgba): void {
  const minX = Math.max(0, Math.floor(Math.min(...polygon.map((point) => point.x))));
  const maxX = Math.min(target.width - 1, Math.ceil(Math.max(...polygon.map((point) => point.x))));
  const minY = Math.max(0, Math.floor(Math.min(...polygon.map((point) => point.y))));
  const maxY = Math.min(target.height - 1, Math.ceil(Math.max(...polygon.map((point) => point.y))));

  for (let y = minY; y <= maxY; y += 1) {
    for (let x = minX; x <= maxX; x += 1) {
      if (pointInPolygon(x + 0.5, y + 0.5, polygon)) {
        blendPixel(target.data, target.width, x, y, color, 1);
      }
    }
  }
}

function pointInPolygon(x: number, y: number, polygon: Point[]): boolean {
  let inside = false;
  for (let current = 0, previous = polygon.length - 1; current < polygon.length; previous = current, current += 1) {
    const currentPoint = polygon[current]!;
    const previousPoint = polygon[previous]!;
    const crosses = currentPoint.y > y !== previousPoint.y > y;
    if (!crosses) {
      continue;
    }
    const intersectionX =
      ((previousPoint.x - currentPoint.x) * (y - currentPoint.y)) / (previousPoint.y - currentPoint.y) +
      currentPoint.x;
    if (x < intersectionX) {
      inside = !inside;
    }
  }
  return inside;
}

function drawLine(target: RgbaImage, from: Point, to: Point, color: Rgba): void {
  const steps = Math.max(Math.abs(to.x - from.x), Math.abs(to.y - from.y));
  for (let step = 0; step <= steps; step += 1) {
    const alpha = steps === 0 ? 0 : step / steps;
    const x = Math.round(from.x + (to.x - from.x) * alpha);
    const y = Math.round(from.y + (to.y - from.y) * alpha);
    if (x >= 0 && x < target.width && y >= 0 && y < target.height) {
      writePixel(target.data, target.width, x, y, color);
    }
  }
}

function previewTexture(texture: RenderedTexture): RgbaImage {
  if (!texture.tint) {
    return texture;
  }
  return applyRoleTint(texture, parseHexColor(texture.tint.normal));
}

function applyRoleTint(texture: RenderedTexture, tint: Rgba): RgbaImage {
  if (texture.source !== "tintable") {
    return texture;
  }
  return tintTexture(texture, tint);
}

function drawTintSwatch(target: RgbaImage, x: number, y: number, color: Rgba): void {
  drawRect(target, x, y, 64, 20, [24, 26, 26, 255]);
  drawRect(target, x + 2, y + 2, 60, 16, color);
}
