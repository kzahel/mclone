import type {
  GeneratedChunkLifecycleRecord,
  GeneratedChunkLifecycleSnapshot,
} from "../../runtime/protocol/chunk-lifecycle";
import type { GuiDrawList } from "../../renderer/gui/gui-draw-list";
import { GuiComponent } from "./gui-component";
import type { Font } from "./font";

export type ChunkLifecycleHudCellState =
  | "unload"
  | "dirty"
  | "published"
  | "blocked"
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

interface ChunkLifecycleHudLayout extends ChunkLifecycleHudPanel {
  readonly bounds: ChunkLifecycleHudBounds;
  readonly cellSize: number;
  readonly headerLines: readonly string[];
  readonly legendRows: readonly (readonly ChunkLifecycleHudLegendItem[])[];
  readonly gridHeight: number;
}

interface ChunkLifecycleHudPlaceholderLayout extends ChunkLifecycleHudPanel {
  readonly lines: readonly string[];
}

interface ChunkLifecycleHudLegendItem {
  readonly state: ChunkLifecycleHudCellState;
  readonly label: string;
}

export const CHUNK_LIFECYCLE_HUD_CELL_COLORS: Readonly<Record<ChunkLifecycleHudCellState, number>> = {
  unload: 0xffab47bc,
  dirty: 0xffffb74d,
  published: 0xff4caf50,
  blocked: 0xffef5350,
  ready: 0xff26c6da,
  materialized: 0xff42a5f5,
  generated: 0xff8d8f96,
  empty: 0xff25272b,
};

const CHUNK_LIFECYCLE_HUD_LEGEND_ITEMS: readonly ChunkLifecycleHudLegendItem[] = [
  { state: "published", label: "published" },
  { state: "blocked", label: "blocked" },
  { state: "ready", label: "ready" },
  { state: "materialized", label: "materialized" },
  { state: "generated", label: "generated" },
  { state: "dirty", label: "dirty" },
  { state: "unload", label: "unload" },
];

const LEGEND_SWATCH_SIZE = 5;
const LEGEND_SWATCH_TEXT_GAP = 3;
const LEGEND_ITEM_GAP = 10;
const MAX_HUD_PANEL_WIDTH = 280;

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
    if (record.publicationBlocker.kind === "ready_to_publish") {
      return "ready";
    }
    if (
      record.publicationBlocker.kind !== "outside_publish_view"
      && record.publicationBlocker.kind !== "already_published"
      && record.publicationBlocker.kind !== "dirty_published"
    ) {
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
): ChunkLifecycleHudPanel | undefined {
  const layout = createChunkLifecycleHudLayout(font, guiWidth, guiHeight, snapshot);
  if (layout === undefined || snapshot === undefined) {
    return undefined;
  }

  const { bounds, cellSize, gridHeight, headerLines, legendRows } = layout;
  GuiComponent.fill(drawList, layout.x, layout.y, layout.x + layout.width, layout.y + layout.height, 0x8f000000);

  let y = layout.y + 3;
  for (const line of headerLines) {
    GuiComponent.drawString(drawList, font, line, layout.x + 4, y, 0xffffffff);
    y += font.lineHeight;
  }

  y += 3;
  const gridX = layout.x + 4;
  drawChunkLifecycleGrid(drawList, snapshot, bounds, gridX, y, cellSize);
  y += gridHeight + 4;

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
  const bounds = getChunkLifecycleHudBounds(snapshot);
  if (snapshot === undefined || bounds === undefined) {
    return undefined;
  }

  const columns = bounds.maxChunkX - bounds.minChunkX + 1;
  const rows = bounds.maxChunkZ - bounds.minChunkZ + 1;
  const headerLines = buildChunkLifecycleHudLines(snapshot, bounds);
  const maxPanelWidth = getMaxChunkLifecycleHudPanelWidth(guiWidth);
  const legendRows = createChunkLifecycleLegendRows(font, Math.max(80, maxPanelWidth - 8));
  const headerWidth = headerLines.reduce((width, line) => Math.max(width, font.width(line)), 0);
  const legendWidth = legendRows.reduce((width, row) => Math.max(width, measureChunkLifecycleLegendRow(font, row)), 0);
  const textWidth = Math.max(headerWidth, legendWidth);
  const maxGridWidth = Math.max(48, Math.min(180, maxPanelWidth - 8));
  const maxGridHeight = Math.max(48, Math.min(150, guiHeight - ((headerLines.length + legendRows.length) * font.lineHeight) - 20));
  const cellSize = Math.min(8, Math.max(2, Math.floor(Math.min(maxGridWidth / columns, maxGridHeight / rows))));
  if (cellSize < 2) {
    return undefined;
  }

  const gridWidth = columns * cellSize;
  const gridHeight = rows * cellSize;
  const panelWidth = Math.min(maxPanelWidth, Math.max(gridWidth + 8, textWidth + 8));
  const panelHeight = 6
    + (headerLines.length * font.lineHeight)
    + 4
    + gridHeight
    + 4
    + (legendRows.length * font.lineHeight)
    + 4;
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
    gridHeight,
  };
}

function getMaxChunkLifecycleHudPanelWidth(guiWidth: number): number {
  return Math.max(80, Math.min(MAX_HUD_PANEL_WIDTH, guiWidth - 4));
}

function createChunkLifecycleHudPlaceholderLayout(
  font: Font,
  guiWidth: number,
  guiHeight: number,
  lines: readonly string[],
): ChunkLifecycleHudPlaceholderLayout | undefined {
  if (lines.length === 0) {
    return undefined;
  }

  const textWidth = lines.reduce((width, line) => Math.max(width, font.width(line)), 0);
  const panelWidth = Math.min(guiWidth - 4, textWidth + 8);
  const panelHeight = 6 + (lines.length * font.lineHeight) + 4;
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
      GuiComponent.fill(drawList, x, y, x + cellSize - 1, y + cellSize - 1, color);
    }
  }

  drawChunkLifecycleViewOutline(drawList, snapshot, bounds, gridX, gridY, cellSize);
  drawChunkLifecycleCenterMarker(drawList, snapshot, bounds, gridX, gridY, cellSize);
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

function measureChunkLifecycleLegendRow(font: Font, row: readonly ChunkLifecycleHudLegendItem[]): number {
  return row.reduce((width, item, index) => (
    width + (index === 0 ? 0 : LEGEND_ITEM_GAP) + measureChunkLifecycleLegendItem(font, item)
  ), 0);
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

function chunkLifecycleHudKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

function drawRectOutline(drawList: GuiDrawList, x0: number, y0: number, x1: number, y1: number, color: number): void {
  GuiComponent.fill(drawList, x0, y0, x1 + 1, y0 + 1, color);
  GuiComponent.fill(drawList, x0, y1, x1 + 1, y1 + 1, color);
  GuiComponent.fill(drawList, x0, y0, x0 + 1, y1 + 1, color);
  GuiComponent.fill(drawList, x1, y0, x1 + 1, y1 + 1, color);
}
