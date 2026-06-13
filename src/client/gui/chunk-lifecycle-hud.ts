import type {
  GeneratedChunkLifecycleRecord,
  GeneratedChunkLifecycleSnapshot,
} from "../../runtime/protocol/chunk-lifecycle";
import type { GuiDrawList } from "../../renderer/gui/gui-draw-list";
import { GuiComponent } from "./gui-component";
import type { Font } from "./font";
import type { GuiRenderMetrics } from "./gui-render-metrics";

export type ChunkLifecycleHudCellState =
  | "unload"
  | "dirty"
  | "published"
  | "blocked"
  | "blocked_missing"
  | "blocked_neighbor"
  | "blocked_light"
  | "ready"
  | "materialized"
  | "generated"
  | "empty";

export interface ChunkLifecycleHudBounds {
  readonly minChunkX: number;
  readonly maxChunkX: number;
  readonly minChunkZ: number;
  readonly maxChunkZ: number;
}

export interface ChunkLifecycleHudPanel {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export interface ChunkLifecycleHudMarker {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly yawDeg?: number;
}

interface ChunkLifecycleHudLayout extends ChunkLifecycleHudPanel {
  readonly bounds: ChunkLifecycleHudBounds;
  readonly cellSize: number;
  readonly headerLines: readonly string[];
  readonly legendRows: readonly (readonly ChunkLifecycleHudLegendItem[])[];
  readonly gridBoxWidth: number;
  readonly gridBoxHeight: number;
  readonly gridWidth: number;
  readonly gridHeight: number;
}

interface ChunkLifecycleHudPlaceholderLayout extends ChunkLifecycleHudPanel {
  readonly lines: readonly string[];
}

interface ChunkLifecycleHudLegendItem {
  readonly state: ChunkLifecycleHudCellState;
  readonly label: string;
}

interface ChunkLifecycleHudRenderOptions {
  readonly metrics?: GuiRenderMetrics;
  readonly marker?: ChunkLifecycleHudMarker;
}

export const CHUNK_LIFECYCLE_HUD_CELL_COLORS: Readonly<Record<ChunkLifecycleHudCellState, number>> = {
  unload: 0xffab47bc,
  dirty: 0xffffb74d,
  published: 0xff4caf50,
  blocked: 0xffef5350,
  blocked_missing: 0xffef5350,
  blocked_neighbor: 0xffff7043,
  blocked_light: 0xffffd54f,
  ready: 0xff26c6da,
  materialized: 0xff42a5f5,
  generated: 0xff8d8f96,
  empty: 0xff303030,
};

const CHUNK_LIFECYCLE_HUD_LEGEND_ITEMS: readonly ChunkLifecycleHudLegendItem[] = [
  { state: "published", label: "published" },
  { state: "blocked_missing", label: "missing" },
  { state: "blocked_neighbor", label: "neighbor" },
  { state: "blocked_light", label: "light" },
  { state: "ready", label: "ready" },
  { state: "materialized", label: "materialized" },
  { state: "generated", label: "generated" },
  { state: "empty", label: "empty" },
  { state: "dirty", label: "dirty" },
  { state: "unload", label: "unload" },
];

const LEGEND_SWATCH_SIZE = 5;
const LEGEND_SWATCH_TEXT_GAP = 3;
const LEGEND_ITEM_GAP = 10;
const MAX_HUD_PANEL_WIDTH = 260;
const MAX_HUD_PANEL_HEIGHT = 210;
const MAX_HUD_GRID_CHUNKS_PER_AXIS = 41;
const GRID_CELL_GAP_GUI_UNITS = 1;
const GRID_CELL_GAP_SCREEN_PIXELS = 1;
const PLAYER_MARKER_COLOR = 0xfffff176;
const PLAYER_MARKER_SHADOW_COLOR = 0xff111111;

export function getChunkLifecycleHudCellState(record: GeneratedChunkLifecycleRecord): ChunkLifecycleHudCellState {
  if (record.queuedForUnload || record.pendingUnload) {
    return "unload";
  }
  if (record.dirtyForPublication || record.dirtyDurable || record.pendingStorageWrite) {
    return "dirty";
  }
  if (record.published) {
    return "published";
  }
  if (record.inPublishView) {
    switch (record.publicationBlocker.kind) {
      case "ready_to_publish":
        return "ready";
      case "missing_materialized_chunk":
        return "blocked_missing";
      case "waiting_for_full_neighbor":
        return "blocked_neighbor";
      case "waiting_for_light":
        return "blocked_light";
      case "outside_publish_view":
      case "already_published":
      case "dirty_published":
        break;
      default:
        return "blocked";
    }
  }
  if (record.hasBlockSections || record.chunkLoaded) {
    return "materialized";
  }
  if (record.generatedStatus !== "empty" || record.ticketSources.length > 0 || record.holderFullStatus !== undefined) {
    return "generated";
  }
  return "empty";
}

export function getChunkLifecycleHudBounds(
  snapshot: GeneratedChunkLifecycleSnapshot | undefined,
): ChunkLifecycleHudBounds | undefined {
  if (snapshot === undefined || snapshot.records.length === 0) {
    return undefined;
  }

  let minChunkX = Number.POSITIVE_INFINITY;
  let maxChunkX = Number.NEGATIVE_INFINITY;
  let minChunkZ = Number.POSITIVE_INFINITY;
  let maxChunkZ = Number.NEGATIVE_INFINITY;
  for (const record of snapshot.records) {
    minChunkX = Math.min(minChunkX, record.chunkX);
    maxChunkX = Math.max(maxChunkX, record.chunkX);
    minChunkZ = Math.min(minChunkZ, record.chunkZ);
    maxChunkZ = Math.max(maxChunkZ, record.chunkZ);
  }

  return { minChunkX, maxChunkX, minChunkZ, maxChunkZ };
}

export function buildChunkLifecycleHudLines(
  snapshot: GeneratedChunkLifecycleSnapshot,
  bounds: ChunkLifecycleHudBounds,
): string[] {
  const blocked = Object.entries(snapshot.counts.byPublicationBlocker)
    .filter(([kind]) => (
      kind !== "outside_publish_view"
      && kind !== "already_published"
      && kind !== "dirty_published"
      && kind !== "ready_to_publish"
    ))
    .reduce((sum, [, count]) => sum + count, 0);
  const activeRevision = snapshot.activeChunkViewJobRevision === undefined
    ? ""
    : ` active=${snapshot.activeChunkViewJobRevision.toString()}`;
  const view = snapshot.currentChunkView === undefined
    ? "view=n/a"
    : `view=${snapshot.currentChunkView.centerChunkX.toString()},${snapshot.currentChunkView.centerChunkZ.toString()} r=${snapshot.currentChunkView.radius.toString()}`;

  return [
    `Chunk Lifecycle rev=${snapshot.chunkViewJobRevision.toString()}${activeRevision}`,
    `x=${bounds.minChunkX.toString()}..${bounds.maxChunkX.toString()} z=${bounds.minChunkZ.toString()}..${bounds.maxChunkZ.toString()}`,
    `${view} pub=${snapshot.counts.published.toString()}/${snapshot.counts.inPublishView.toString()} loaded=${snapshot.counts.loaded.toString()} blocked=${blocked.toString()}`,
  ];
}

export function renderChunkLifecycleHud(
  drawList: GuiDrawList,
  font: Font,
  guiWidth: number,
  guiHeight: number,
  snapshot: GeneratedChunkLifecycleSnapshot | undefined,
  options: ChunkLifecycleHudRenderOptions = {},
): ChunkLifecycleHudPanel | undefined {
  const layout = createChunkLifecycleHudLayout(font, guiWidth, guiHeight, snapshot);
  if (layout === undefined || snapshot === undefined) {
    return undefined;
  }

  const { bounds, cellSize, gridBoxWidth, gridBoxHeight, gridWidth, gridHeight, headerLines, legendRows } = layout;
  GuiComponent.fill(drawList, layout.x, layout.y, layout.x + layout.width, layout.y + layout.height, 0x8f000000);

  let y = layout.y + 3;
  for (const line of headerLines) {
    GuiComponent.drawString(drawList, font, line, layout.x + 4, y, 0xffffffff);
    y += font.lineHeight;
  }

  y += 3;
  const gridBoxX = layout.x + 4;
  const gridBoxY = y;
  const gridX = gridBoxX + Math.floor((gridBoxWidth - gridWidth) / 2);
  y = gridBoxY + Math.floor((gridBoxHeight - gridHeight) / 2);
  drawChunkLifecycleGrid(drawList, snapshot, bounds, gridX, y, cellSize, options);
  y = gridBoxY + gridBoxHeight + 4;

  for (const row of legendRows) {
    drawChunkLifecycleLegendRow(drawList, font, row, layout.x + 4, y);
    y += font.lineHeight;
  }

  return {
    x: layout.x,
    y: layout.y,
    width: layout.width,
    height: layout.height,
  };
}

export function renderChunkLifecycleHudPlaceholder(
  drawList: GuiDrawList,
  font: Font,
  guiWidth: number,
  guiHeight: number,
  lines: readonly string[],
): ChunkLifecycleHudPanel | undefined {
  const layout = createChunkLifecycleHudPlaceholderLayout(font, guiWidth, guiHeight, lines);
  if (layout === undefined) {
    return undefined;
  }

  GuiComponent.fill(drawList, layout.x, layout.y, layout.x + layout.width, layout.y + layout.height, 0x8f000000);
  let y = layout.y + 3;
  for (let index = 0; index < layout.lines.length; index++) {
    GuiComponent.drawString(
      drawList,
      font,
      layout.lines[index]!,
      layout.x + 4,
      y,
      index === 0 ? 0xffffffff : 0xffd8dee9,
    );
    y += font.lineHeight;
  }

  return {
    x: layout.x,
    y: layout.y,
    width: layout.width,
    height: layout.height,
  };
}

export function measureChunkLifecycleHudPanel(
  font: Font,
  guiWidth: number,
  guiHeight: number,
  snapshot: GeneratedChunkLifecycleSnapshot | undefined,
): ChunkLifecycleHudPanel | undefined {
  const layout = createChunkLifecycleHudLayout(font, guiWidth, guiHeight, snapshot);
  return layout === undefined
    ? undefined
    : {
      x: layout.x,
      y: layout.y,
      width: layout.width,
      height: layout.height,
    };
}

export function measureChunkLifecycleHudPlaceholderPanel(
  font: Font,
  guiWidth: number,
  guiHeight: number,
  lines: readonly string[],
): ChunkLifecycleHudPanel | undefined {
  const layout = createChunkLifecycleHudPlaceholderLayout(font, guiWidth, guiHeight, lines);
  return layout === undefined
    ? undefined
    : {
      x: layout.x,
      y: layout.y,
      width: layout.width,
      height: layout.height,
    };
}

function createChunkLifecycleHudLayout(
  font: Font,
  guiWidth: number,
  guiHeight: number,
  snapshot: GeneratedChunkLifecycleSnapshot | undefined,
): ChunkLifecycleHudLayout | undefined {
  const rawBounds = getChunkLifecycleHudBounds(snapshot);
  if (snapshot === undefined || rawBounds === undefined) {
    return undefined;
  }

  const bounds = clampChunkLifecycleHudBounds(rawBounds, snapshot.currentChunkView);
  const columns = bounds.maxChunkX - bounds.minChunkX + 1;
  const rows = bounds.maxChunkZ - bounds.minChunkZ + 1;
  const headerLines = buildChunkLifecycleHudLines(snapshot, bounds);
  const maxPanelWidth = getMaxChunkLifecycleHudPanelWidth(guiWidth);
  const panelWidth = maxPanelWidth;
  const panelHeight = getChunkLifecycleHudPanelHeight(guiHeight);
  const legendRows = createChunkLifecycleLegendRows(font, Math.max(80, panelWidth - 8));
  const gridBoxWidth = Math.max(48, Math.min(180, panelWidth - 8));
  const gridBoxHeight = getChunkLifecycleHudGridBoxHeight(font, panelHeight, headerLines.length, legendRows.length);
  const cellSize = Math.min(8, Math.max(2, Math.floor(Math.min(gridBoxWidth / columns, gridBoxHeight / rows))));
  if (cellSize < 2) {
    return undefined;
  }

  const gridWidth = columns * cellSize;
  const gridHeight = rows * cellSize;
  if (panelWidth <= 0 || panelHeight > guiHeight) {
    return undefined;
  }

  const panelX = Math.max(0, guiWidth - panelWidth - 2);
  const panelY = 0;
  return {
    x: panelX,
    y: panelY,
    width: panelWidth,
    height: panelHeight,
    bounds,
    cellSize,
    headerLines,
    legendRows,
    gridBoxWidth,
    gridBoxHeight,
    gridWidth,
    gridHeight,
  };
}

function getMaxChunkLifecycleHudPanelWidth(guiWidth: number): number {
  return Math.max(80, Math.min(MAX_HUD_PANEL_WIDTH, guiWidth - 4));
}

function getChunkLifecycleHudPanelHeight(guiHeight: number): number {
  return Math.max(80, Math.min(MAX_HUD_PANEL_HEIGHT, guiHeight));
}

function getChunkLifecycleHudGridBoxHeight(
  font: Font,
  panelHeight: number,
  headerLineCount: number,
  legendRowCount: number,
): number {
  const reservedHeight = 6
    + (headerLineCount * font.lineHeight)
    + 4
    + 4
    + (legendRowCount * font.lineHeight)
    + 4;
  return Math.max(16, panelHeight - reservedHeight);
}

function createChunkLifecycleHudPlaceholderLayout(
  _font: Font,
  guiWidth: number,
  guiHeight: number,
  lines: readonly string[],
): ChunkLifecycleHudPlaceholderLayout | undefined {
  if (lines.length === 0) {
    return undefined;
  }

  const panelWidth = getMaxChunkLifecycleHudPanelWidth(guiWidth);
  const panelHeight = getChunkLifecycleHudPanelHeight(guiHeight);
  if (panelWidth <= 0 || panelHeight > guiHeight) {
    return undefined;
  }

  return {
    x: Math.max(0, guiWidth - panelWidth - 2),
    y: 0,
    width: panelWidth,
    height: panelHeight,
    lines,
  };
}

function drawChunkLifecycleGrid(
  drawList: GuiDrawList,
  snapshot: GeneratedChunkLifecycleSnapshot,
  bounds: ChunkLifecycleHudBounds,
  gridX: number,
  gridY: number,
  cellSize: number,
  options: ChunkLifecycleHudRenderOptions,
): void {
  const records = new Map<string, GeneratedChunkLifecycleRecord>();
  for (const record of snapshot.records) {
    records.set(chunkLifecycleHudKey(record.chunkX, record.chunkZ), record);
  }

  for (let chunkZ = bounds.minChunkZ; chunkZ <= bounds.maxChunkZ; chunkZ++) {
    for (let chunkX = bounds.minChunkX; chunkX <= bounds.maxChunkX; chunkX++) {
      const record = records.get(chunkLifecycleHudKey(chunkX, chunkZ));
      const color = record === undefined
        ? CHUNK_LIFECYCLE_HUD_CELL_COLORS.empty
        : CHUNK_LIFECYCLE_HUD_CELL_COLORS[getChunkLifecycleHudCellState(record)];
      const x = gridX + ((chunkX - bounds.minChunkX) * cellSize);
      const y = gridY + ((chunkZ - bounds.minChunkZ) * cellSize);
      drawChunkLifecycleCell(drawList, x, y, cellSize, color, options.metrics);
    }
  }

  drawChunkLifecycleViewOutline(drawList, snapshot, bounds, gridX, gridY, cellSize);
  drawChunkLifecycleCenterMarker(drawList, snapshot, bounds, gridX, gridY, cellSize);
  drawChunkLifecycleMarker(drawList, options.marker, bounds, gridX, gridY, cellSize);
}

function clampChunkLifecycleHudBounds(
  bounds: ChunkLifecycleHudBounds,
  view: GeneratedChunkLifecycleSnapshot["currentChunkView"],
): ChunkLifecycleHudBounds {
  if (view === undefined) {
    return bounds;
  }

  const [minChunkX, maxChunkX] = clampChunkLifecycleHudAxis(
    bounds.minChunkX,
    bounds.maxChunkX,
    view.centerChunkX,
  );
  const [minChunkZ, maxChunkZ] = clampChunkLifecycleHudAxis(
    bounds.minChunkZ,
    bounds.maxChunkZ,
    view.centerChunkZ,
  );
  return { minChunkX, maxChunkX, minChunkZ, maxChunkZ };
}

function clampChunkLifecycleHudAxis(min: number, max: number, center: number): readonly [number, number] {
  if (max - min + 1 <= MAX_HUD_GRID_CHUNKS_PER_AXIS) {
    return [min, max];
  }

  let nextMin = center - Math.floor(MAX_HUD_GRID_CHUNKS_PER_AXIS / 2);
  let nextMax = nextMin + MAX_HUD_GRID_CHUNKS_PER_AXIS - 1;
  if (nextMin < min) {
    nextMin = min;
    nextMax = nextMin + MAX_HUD_GRID_CHUNKS_PER_AXIS - 1;
  }
  if (nextMax > max) {
    nextMax = max;
    nextMin = nextMax - MAX_HUD_GRID_CHUNKS_PER_AXIS + 1;
  }
  return [nextMin, nextMax];
}

function drawChunkLifecycleCell(
  drawList: GuiDrawList,
  x: number,
  y: number,
  cellSize: number,
  color: number,
  metrics: GuiRenderMetrics | undefined,
): void {
  if (metrics === undefined || metrics.pixelScaleX <= 0 || metrics.pixelScaleY <= 0) {
    GuiComponent.fill(
      drawList,
      x,
      y,
      x + cellSize - GRID_CELL_GAP_GUI_UNITS,
      y + cellSize - GRID_CELL_GAP_GUI_UNITS,
      color,
    );
    return;
  }

  const x0Px = Math.round(x * metrics.pixelScaleX);
  const y0Px = Math.round(y * metrics.pixelScaleY);
  const x1Px = Math.round((x + cellSize) * metrics.pixelScaleX) - GRID_CELL_GAP_SCREEN_PIXELS;
  const y1Px = Math.round((y + cellSize) * metrics.pixelScaleY) - GRID_CELL_GAP_SCREEN_PIXELS;
  if (x1Px <= x0Px || y1Px <= y0Px) {
    return;
  }

  GuiComponent.fill(
    drawList,
    x0Px / metrics.pixelScaleX,
    y0Px / metrics.pixelScaleY,
    x1Px / metrics.pixelScaleX,
    y1Px / metrics.pixelScaleY,
    color,
  );
}

function createChunkLifecycleLegendRows(
  font: Font,
  maxWidth: number,
): readonly (readonly ChunkLifecycleHudLegendItem[])[] {
  const rows: ChunkLifecycleHudLegendItem[][] = [];
  let row: ChunkLifecycleHudLegendItem[] = [];
  let rowWidth = 0;
  for (const item of CHUNK_LIFECYCLE_HUD_LEGEND_ITEMS) {
    const itemWidth = measureChunkLifecycleLegendItem(font, item);
    const nextWidth = row.length === 0 ? itemWidth : rowWidth + LEGEND_ITEM_GAP + itemWidth;
    if (row.length > 0 && nextWidth > maxWidth) {
      rows.push(row);
      row = [item];
      rowWidth = itemWidth;
    } else {
      row.push(item);
      rowWidth = nextWidth;
    }
  }

  if (row.length > 0) {
    rows.push(row);
  }

  return rows;
}

function measureChunkLifecycleLegendItem(font: Font, item: ChunkLifecycleHudLegendItem): number {
  return LEGEND_SWATCH_SIZE + LEGEND_SWATCH_TEXT_GAP + font.width(item.label);
}

function drawChunkLifecycleLegendRow(
  drawList: GuiDrawList,
  font: Font,
  row: readonly ChunkLifecycleHudLegendItem[],
  x: number,
  y: number,
): void {
  let cursorX = x;
  const swatchY = y + Math.max(1, Math.floor((font.lineHeight - LEGEND_SWATCH_SIZE) / 2));
  for (const item of row) {
    GuiComponent.fill(
      drawList,
      cursorX,
      swatchY,
      cursorX + LEGEND_SWATCH_SIZE,
      swatchY + LEGEND_SWATCH_SIZE,
      CHUNK_LIFECYCLE_HUD_CELL_COLORS[item.state],
    );
    cursorX += LEGEND_SWATCH_SIZE + LEGEND_SWATCH_TEXT_GAP;
    GuiComponent.drawString(drawList, font, item.label, cursorX, y, 0xffd8dee9);
    cursorX += font.width(item.label) + LEGEND_ITEM_GAP;
  }
}

function drawChunkLifecycleViewOutline(
  drawList: GuiDrawList,
  snapshot: GeneratedChunkLifecycleSnapshot,
  bounds: ChunkLifecycleHudBounds,
  gridX: number,
  gridY: number,
  cellSize: number,
): void {
  const view = snapshot.currentChunkView;
  if (view === undefined) {
    return;
  }

  const minChunkX = Math.max(bounds.minChunkX, view.centerChunkX - view.radius);
  const maxChunkX = Math.min(bounds.maxChunkX, view.centerChunkX + view.radius);
  const minChunkZ = Math.max(bounds.minChunkZ, view.centerChunkZ - view.radius);
  const maxChunkZ = Math.min(bounds.maxChunkZ, view.centerChunkZ + view.radius);
  if (minChunkX > maxChunkX || minChunkZ > maxChunkZ) {
    return;
  }

  const x0 = gridX + ((minChunkX - bounds.minChunkX) * cellSize);
  const y0 = gridY + ((minChunkZ - bounds.minChunkZ) * cellSize);
  const x1 = gridX + ((maxChunkX - bounds.minChunkX + 1) * cellSize) - 1;
  const y1 = gridY + ((maxChunkZ - bounds.minChunkZ + 1) * cellSize) - 1;
  drawRectOutline(drawList, x0, y0, x1, y1, 0xb0ffffff);
}

function drawChunkLifecycleCenterMarker(
  drawList: GuiDrawList,
  snapshot: GeneratedChunkLifecycleSnapshot,
  bounds: ChunkLifecycleHudBounds,
  gridX: number,
  gridY: number,
  cellSize: number,
): void {
  const view = snapshot.currentChunkView;
  if (
    view === undefined
    || view.centerChunkX < bounds.minChunkX
    || view.centerChunkX > bounds.maxChunkX
    || view.centerChunkZ < bounds.minChunkZ
    || view.centerChunkZ > bounds.maxChunkZ
  ) {
    return;
  }

  const x = gridX + ((view.centerChunkX - bounds.minChunkX) * cellSize);
  const y = gridY + ((view.centerChunkZ - bounds.minChunkZ) * cellSize);
  if (cellSize <= 3) {
    GuiComponent.fill(drawList, x, y, x + cellSize, y + cellSize, 0xffffffff);
    return;
  }

  const inset = Math.max(1, Math.floor(cellSize / 3));
  GuiComponent.fill(drawList, x + inset, y + inset, x + cellSize - inset, y + cellSize - inset, 0xffffffff);
}

function drawChunkLifecycleMarker(
  drawList: GuiDrawList,
  marker: ChunkLifecycleHudMarker | undefined,
  bounds: ChunkLifecycleHudBounds,
  gridX: number,
  gridY: number,
  cellSize: number,
): void {
  if (
    marker === undefined
    || marker.chunkX < bounds.minChunkX
    || marker.chunkX > bounds.maxChunkX
    || marker.chunkZ < bounds.minChunkZ
    || marker.chunkZ > bounds.maxChunkZ
  ) {
    return;
  }

  const centerX = gridX + ((marker.chunkX - bounds.minChunkX) * cellSize) + (cellSize / 2);
  const centerY = gridY + ((marker.chunkZ - bounds.minChunkZ) * cellSize) + (cellSize / 2);
  const radius = Math.max(2, Math.min(4, Math.floor(cellSize / 2)));
  drawPixelLine(
    drawList,
    Math.round(centerX - radius),
    Math.round(centerY),
    Math.round(centerX + radius),
    Math.round(centerY),
    PLAYER_MARKER_SHADOW_COLOR,
  );
  drawPixelLine(
    drawList,
    Math.round(centerX),
    Math.round(centerY - radius),
    Math.round(centerX),
    Math.round(centerY + radius),
    PLAYER_MARKER_SHADOW_COLOR,
  );

  const yawDeg = marker.yawDeg;
  if (yawDeg === undefined || !Number.isFinite(yawDeg)) {
    GuiComponent.fill(drawList, centerX - 1, centerY - 1, centerX + 2, centerY + 2, PLAYER_MARKER_COLOR);
    return;
  }

  const yawRad = yawDeg * Math.PI / 180.0;
  const dx = -Math.sin(yawRad);
  const dy = Math.cos(yawRad);
  const length = Math.max(5, cellSize * 1.75);
  const tipX = centerX + (dx * length);
  const tipY = centerY + (dy * length);
  drawPixelLine(drawList, Math.round(centerX), Math.round(centerY), Math.round(tipX), Math.round(tipY), PLAYER_MARKER_SHADOW_COLOR, 3);
  drawPixelLine(drawList, Math.round(centerX), Math.round(centerY), Math.round(tipX), Math.round(tipY), PLAYER_MARKER_COLOR);
  drawPlayerMarkerHead(drawList, tipX, tipY, dx, dy);
}

function drawPlayerMarkerHead(
  drawList: GuiDrawList,
  tipX: number,
  tipY: number,
  dx: number,
  dy: number,
): void {
  const headLength = 3.5;
  const sideX = -dy;
  const sideY = dx;
  const baseX = tipX - (dx * headLength);
  const baseY = tipY - (dy * headLength);
  drawPixelLine(
    drawList,
    Math.round(tipX),
    Math.round(tipY),
    Math.round(baseX + (sideX * headLength * 0.7)),
    Math.round(baseY + (sideY * headLength * 0.7)),
    PLAYER_MARKER_COLOR,
  );
  drawPixelLine(
    drawList,
    Math.round(tipX),
    Math.round(tipY),
    Math.round(baseX - (sideX * headLength * 0.7)),
    Math.round(baseY - (sideY * headLength * 0.7)),
    PLAYER_MARKER_COLOR,
  );
}

function drawPixelLine(
  drawList: GuiDrawList,
  x0: number,
  y0: number,
  x1: number,
  y1: number,
  color: number,
  thickness = 1,
): void {
  const dx = Math.abs(x1 - x0);
  const sx = x0 < x1 ? 1 : -1;
  const dy = -Math.abs(y1 - y0);
  const sy = y0 < y1 ? 1 : -1;
  let err = dx + dy;
  let x = x0;
  let y = y0;
  const half = Math.floor(thickness / 2);

  while (true) {
    GuiComponent.fill(drawList, x - half, y - half, x + half + 1, y + half + 1, color);
    if (x === x1 && y === y1) {
      break;
    }
    const e2 = 2 * err;
    if (e2 >= dy) {
      err += dy;
      x += sx;
    }
    if (e2 <= dx) {
      err += dx;
      y += sy;
    }
  }
}

function chunkLifecycleHudKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

function drawRectOutline(drawList: GuiDrawList, x0: number, y0: number, x1: number, y1: number, color: number): void {
  GuiComponent.fill(drawList, x0, y0, x1 + 1, y0 + 1, color);
  GuiComponent.fill(drawList, x0, y1, x1 + 1, y1 + 1, color);
  GuiComponent.fill(drawList, x0, y0, x0 + 1, y1 + 1, color);
  GuiComponent.fill(drawList, x1, y0, x1 + 1, y1 + 1, color);
}
