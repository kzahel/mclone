import { useEffect, useMemo, useRef, useState } from "react";

import type { TerrainLabCamera, TerrainLabState } from "../state";
import { initializeTerrainLab } from "./terrain-lab-wasm";
import { useWorldViewNavigation } from "./use-world-view-navigation";
import type {
  WildlifePopulationCellReceipt,
  WildlifePopulationSummary,
  WildlifePopulationWorkerResponse,
  WildlifeSpecies,
} from "./wildlife-population-worker-protocol";

interface CanvasSize {
  width: number;
  height: number;
  cssWidth: number;
  cssHeight: number;
  dpr: number;
}

export interface WildlifePopulationReport {
  seed: string;
  schema: string;
  revision: number;
  buildMs: number;
  checksum: string;
  cellCount: number;
  occupiedCells: number;
  animalCount: number;
  speciesCounts: [number, number, number, number];
}

interface WildlifePopulationCanvasProps {
  state: TerrainLabState;
  camera: TerrainLabCamera;
  onStateChange: (state: TerrainLabState) => void;
  onCameraChange: (camera: TerrainLabCamera) => void;
  onReport: (report: WildlifePopulationReport | undefined) => void;
  onInspect: (receipt: WildlifePopulationCellReceipt | undefined) => void;
  onError: (error: string | undefined) => void;
}

const SPECIES: WildlifeSpecies[] = ["rabbit", "deer", "mallard", "bee"];
const HABITATS = [
  ["ocean", "Ocean"],
  ["river", "River"],
  ["shore", "Shore"],
  ["snowyAlpine", "Alpine"],
  ["coolWetConifer", "Conifer"],
  ["warmDrySteppe", "Steppe"],
  ["temperateWoodland", "Woodland"],
  ["temperateMeadow", "Meadow"],
] as const;

export function WildlifePopulationCanvas({
  state,
  camera,
  onStateChange,
  onCameraChange,
  onReport,
  onInspect,
  onError,
}: WildlifePopulationCanvasProps): React.JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const workerRef = useRef<Worker | undefined>(undefined);
  const epochRef = useRef(0);
  const [navigationReady, setNavigationReady] = useState(false);
  const [summary, setSummary] = useState<WildlifePopulationSummary>();
  const [buildMs, setBuildMs] = useState(0);
  const [selected, setSelected] = useState<WildlifePopulationCellReceipt>();
  const [canvasSize, setCanvasSize] = useState<CanvasSize>({
    width: 900,
    height: 700,
    cssWidth: 900,
    cssHeight: 700,
    dpr: 1,
  });
  const navigationState = useMemo<TerrainLabState>(
    () => ({ ...state, view: "map" }),
    [state],
  );
  const navigation = useWorldViewNavigation({
    stageRef,
    enabled: summary !== undefined && navigationReady,
    state: navigationState,
    camera,
    onStateChange: (next) => onStateChange({
      ...state,
      centerX: next.centerX,
      centerZ: next.centerZ,
      blocksAcross: next.blocksAcross,
    }),
    onCameraChange,
    resolveViewport: (clientX, clientY) => {
      const stage = stageRef.current;
      if (!stage) {
        return undefined;
      }
      const rect = stage.getBoundingClientRect();
      return {
        x: clientX - rect.left,
        y: clientY - rect.top,
        width: rect.width,
        height: rect.height,
      };
    },
    onTap: (clientX, clientY) => {
      const stage = stageRef.current;
      if (!stage || !summary) {
        return;
      }
      const rect = stage.getBoundingClientRect();
      const view = populationView(canvasSize, state);
      const worldX = view.minX
        + (clientX - rect.left) / Math.max(rect.width, 1) * view.width;
      const worldZ = view.minZ
        + (clientY - rect.top) / Math.max(rect.height, 1) * view.height;
      const cellX = Math.floor(worldX / summary.cellBlocks);
      const cellZ = Math.floor(worldZ / summary.cellBlocks);
      const receipt = summary.cells.find((cell) =>
        cell.cellX === cellX && cell.cellZ === cellZ
      );
      setSelected(receipt);
      onInspect(receipt);
    },
  });

  useEffect(() => {
    let cancelled = false;
    void initializeTerrainLab()
      .then(() => {
        if (!cancelled) {
          setNavigationReady(true);
        }
      })
      .catch((error: unknown) => onError(errorMessage(error)));
    return () => {
      cancelled = true;
    };
  }, [onError]);

  useEffect(() => {
    const worker = new Worker(
      new URL("./wildlife-population-worker.ts", import.meta.url),
      { type: "module" },
    );
    workerRef.current = worker;
    worker.onmessage = (
      event: MessageEvent<WildlifePopulationWorkerResponse>,
    ): void => {
      const response = event.data;
      if (response.epoch !== epochRef.current) {
        return;
      }
      if (response.type === "error") {
        onError(response.message);
        return;
      }
      setSummary(response.summary);
      setBuildMs(response.buildMs);
      onReport(reportFromSummary(response.summary, response.buildMs));
    };
    worker.onerror = (event): void => {
      onError(event.message || "Wildlife-population Worker failed.");
    };
    return () => {
      worker.terminate();
      workerRef.current = undefined;
    };
  }, [onError, onReport]);

  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) {
      return;
    }
    const resize = (): void => {
      const rect = stage.getBoundingClientRect();
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      setCanvasSize({
        width: Math.max(1, Math.round(rect.width * dpr)),
        height: Math.max(1, Math.round(rect.height * dpr)),
        cssWidth: Math.max(1, rect.width),
        cssHeight: Math.max(1, rect.height),
        dpr,
      });
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(stage);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const worker = workerRef.current;
    if (!worker) {
      return;
    }
    epochRef.current += 1;
    onReport(undefined);
    worker.postMessage({
      type: "build",
      epoch: epochRef.current,
      seed: state.seed,
      centerX: state.centerX,
      centerZ: state.centerZ,
      blocksAcross: state.blocksAcross,
      aspectRatio: canvasSize.cssWidth / Math.max(canvasSize.cssHeight, 1),
    });
  }, [
    canvasSize.cssHeight,
    canvasSize.cssWidth,
    onInspect,
    onReport,
    state.blocksAcross,
    state.centerX,
    state.centerZ,
    state.seed,
  ]);

  useEffect(() => {
    setSelected(undefined);
    onInspect(undefined);
  }, [onInspect, state.blocksAcross, state.centerX, state.centerZ, state.seed]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }
    const context = canvas.getContext("2d");
    if (!context) {
      onError("This browser does not expose a 2D canvas context.");
      return;
    }
    drawWildlifePopulation(context, canvasSize, state, summary, selected);
  }, [canvasSize, onError, selected, state, summary]);

  return (
    <div className="wildlifePopulationMap">
      <div
        ref={stageRef}
        className="terrainStage wildlifePopulationStage"
        data-testid="wildlife-population-stage"
        data-render-ready={summary ? "true" : "false"}
        data-checksum={summary?.checksum ?? ""}
        data-occupied-cells={summary?.occupiedCells ?? 0}
        data-animal-count={summary?.animalCount ?? 0}
        tabIndex={0}
        aria-keyshortcuts="ArrowUp ArrowDown ArrowLeft ArrowRight"
        onPointerDown={navigation.onPointerDown}
        onPointerMove={navigation.onPointerMove}
        onPointerUp={navigation.onPointerUp}
        onPointerCancel={navigation.onPointerCancel}
        onContextMenu={(event) => event.preventDefault()}
        onKeyDown={navigation.onKeyDown}
      >
        <canvas
          ref={canvasRef}
          className="terrainCanvas"
          width={canvasSize.width}
          height={canvasSize.height}
          aria-label="Deterministic initial wildlife population map"
        />
        <div className="canvasTopline" aria-hidden="true">
          <span className="canvasBadge primary">
            {summary ? `${summary.occupiedCells} encounters` : "planning wildlife"}
          </span>
          <span className="canvasBadge">production · revision {summary?.revision ?? "—"}</span>
          <span className="canvasBadge">
            {summary ? `${summary.animalCount} animals · ${buildMs.toFixed(1)} ms` : "64 m cells"}
          </span>
        </div>
        {selected ? <WildlifeInspector receipt={selected} /> : null}
        <div className="canvasHint">
          Drag to pan · wheel or pinch to zoom · tap a population cell to inspect
        </div>
      </div>
      <WildlifeLegend summary={summary} />
    </div>
  );
}

function WildlifeLegend({
  summary,
}: {
  summary: WildlifePopulationSummary | undefined;
}): React.JSX.Element {
  return (
    <div
      className="wildlifeLegend"
      data-testid="wildlife-map-legend"
      aria-label="Wildlife map legend"
    >
      <section className="wildlifeLegendSection habitatLegend">
        <strong>Cell habitat</strong>
        <div>
          {HABITATS.map(([biome, label]) => (
            <span key={biome}>
              <i
                className="habitatSwatch"
                style={{ background: biomeColor(biome, 650) }}
                aria-hidden="true"
              />
              {label}
            </span>
          ))}
        </div>
      </section>
      <section className="wildlifeLegendSection densityLegend">
        <strong>Desired density tint</strong>
        <div>
          <span>none</span>
          <i aria-hidden="true" />
          <span>higher</span>
        </div>
      </section>
      <section className="wildlifeLegendSection encounterLegend">
        <strong>Encounter result</strong>
        <div>
          {SPECIES.map((species, index) => (
            <span key={species}>
              <i style={{ background: speciesColor(species) }} aria-hidden="true" />
              {species} {summary ? summary.speciesCounts[index] : "—"}
            </span>
          ))}
          <span><i className="empty" aria-hidden="true" /> empty roll</span>
          <span><i className="unsuitable" aria-hidden="true" /> unsuitable</span>
        </div>
      </section>
    </div>
  );
}

function WildlifeInspector({
  receipt,
}: {
  receipt: WildlifePopulationCellReceipt;
}): React.JSX.Element {
  const weights: Array<[string, number]> = [
    ["rabbit", receipt.rabbitWeight],
    ["deer", receipt.deerWeight],
    ["mallard", receipt.mallardWeight],
    ["bee", receipt.beeWeight],
  ];
  return (
    <div className="wildlifeInspector" data-testid="wildlife-cell-inspector">
      <div>
        <span>cell {receipt.cellX}, {receipt.cellZ}</span>
        <strong>
          {receipt.species
            ? `${receipt.groupSize} ${receipt.species}${receipt.groupSize === 1 ? "" : "s"}`
            : receipt.desiredDensity === 0 ? "unsuitable" : "empty this seed"}
        </strong>
      </div>
      <p>{receipt.biome} · {receipt.landform} · surface Y {receipt.surfaceY}</p>
      <dl>
        <div><dt>Density / roll</dt><dd>{receipt.desiredDensity} / {receipt.occupancyRoll}</dd></div>
        <div><dt>Productivity</dt><dd>{receipt.productivity}</dd></div>
        <div><dt>Open / cover</dt><dd>{receipt.openness} / {receipt.forestCover}</dd></div>
        <div><dt>Wetland / water</dt><dd>{receipt.wetland} / {receipt.water}</dd></div>
      </dl>
      <div className="wildlifeWeights">
        {weights.map(([species, value]) => (
          <span key={species}>
            <i style={{ width: `${Math.max(2, value / 10)}%` }} />
            {species} {value}
          </span>
        ))}
      </div>
      <small>
        Selected {receipt.selectedX}, {receipt.selectedZ}
        {receipt.ownerChunkX === null
          ? " · no owning chunk"
          : ` · owner chunk ${receipt.ownerChunkX}, ${receipt.ownerChunkZ}`}
      </small>
    </div>
  );
}

function drawWildlifePopulation(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
  state: TerrainLabState,
  summary: WildlifePopulationSummary | undefined,
  selected: WildlifePopulationCellReceipt | undefined,
): void {
  context.setTransform(1, 0, 0, 1, 0, 0);
  context.fillStyle = "#071012";
  context.fillRect(0, 0, canvas.width, canvas.height);
  if (!summary) {
    drawLoadingGrid(context, canvas);
    return;
  }
  const view = populationView(canvas, state);
  const worldToScreen = (worldX: number, worldZ: number): [number, number] => [
    (worldX - view.minX) / view.width * canvas.width,
    (worldZ - view.minZ) / view.height * canvas.height,
  ];
  const cellPixels = summary.cellBlocks / view.width * canvas.width;
  for (const cell of summary.cells) {
    const worldX = cell.cellX * summary.cellBlocks;
    const worldZ = cell.cellZ * summary.cellBlocks;
    const [x, y] = worldToScreen(worldX, worldZ);
    const [right, bottom] = worldToScreen(
      worldX + summary.cellBlocks,
      worldZ + summary.cellBlocks,
    );
    context.fillStyle = biomeColor(cell.biome, cell.productivity);
    context.fillRect(
      Math.floor(x),
      Math.floor(y),
      Math.ceil(right - x) + 1,
      Math.ceil(bottom - y) + 1,
    );
    if (cell.desiredDensity > 0) {
      context.fillStyle = `rgba(214, 235, 146, ${cell.desiredDensity / 7_000})`;
      context.fillRect(x, y, right - x, bottom - y);
    }
    if (cellPixels >= 5) {
      context.strokeStyle = "rgba(235, 245, 238, 0.13)";
      context.lineWidth = Math.max(0.6, canvas.dpr * 0.55);
      context.strokeRect(x, y, right - x, bottom - y);
    }
    if (!cell.species) {
      const radius = Math.max(1.1 * canvas.dpr, Math.min(3.2 * canvas.dpr, cellPixels * 0.05));
      context.strokeStyle = cell.desiredDensity === 0
        ? "rgba(151, 165, 161, 0.38)"
        : "rgba(235, 245, 238, 0.45)";
      context.lineWidth = Math.max(0.7, canvas.dpr * 0.7);
      context.beginPath();
      context.arc((x + right) * 0.5, (y + bottom) * 0.5, radius, 0, Math.PI * 2);
      context.stroke();
      continue;
    }
    const [anchorX, anchorY] = worldToScreen(cell.selectedX + 0.5, cell.selectedZ + 0.5);
    const radius = Math.max(5 * canvas.dpr, Math.min(13 * canvas.dpr, cellPixels * 0.17));
    context.fillStyle = speciesColor(cell.species);
    context.strokeStyle = "rgba(4, 11, 12, 0.92)";
    context.lineWidth = Math.max(1.2, canvas.dpr * 1.1);
    context.beginPath();
    context.arc(anchorX, anchorY, radius, 0, Math.PI * 2);
    context.fill();
    context.stroke();
    if (radius >= 7 * canvas.dpr) {
      context.fillStyle = "#071012";
      context.font = `700 ${Math.round(radius * 0.95)}px "DM Mono", monospace`;
      context.textAlign = "center";
      context.textBaseline = "middle";
      context.fillText(speciesGlyph(cell.species), anchorX, anchorY + 0.5 * canvas.dpr);
      context.fillStyle = "#f4f7ef";
      context.font = `700 ${Math.round(7 * canvas.dpr)}px "DM Mono", monospace`;
      context.fillText(String(cell.groupSize), anchorX + radius * 0.72, anchorY - radius * 0.72);
    }
  }
  if (selected) {
    const [x, y] = worldToScreen(
      selected.cellX * summary.cellBlocks,
      selected.cellZ * summary.cellBlocks,
    );
    const [right, bottom] = worldToScreen(
      (selected.cellX + 1) * summary.cellBlocks,
      (selected.cellZ + 1) * summary.cellBlocks,
    );
    context.strokeStyle = "#fff4a8";
    context.lineWidth = Math.max(2, canvas.dpr * 2);
    context.strokeRect(x + 1, y + 1, right - x - 2, bottom - y - 2);
  }
}

function biomeColor(biome: string, productivity: number): string {
  const colors: Record<string, [number, number, number]> = {
    ocean: [25, 61, 89],
    shore: [145, 131, 82],
    river: [38, 102, 118],
    snowyAlpine: [161, 177, 174],
    coolWetConifer: [43, 82, 69],
    warmDrySteppe: [125, 119, 67],
    temperateWoodland: [46, 93, 65],
    temperateMeadow: [83, 116, 67],
  };
  const color = colors[biome] ?? [71, 86, 76];
  const lift = productivity / 1_000 * 0.18 + 0.82;
  return `rgb(${Math.round(color[0] * lift)} ${Math.round(color[1] * lift)} ${Math.round(color[2] * lift)})`;
}

function speciesColor(species: WildlifeSpecies): string {
  switch (species) {
    case "rabbit": return "#e4d6bd";
    case "deer": return "#d59458";
    case "mallard": return "#63b9a4";
    case "bee": return "#f2cb4d";
  }
}

function speciesGlyph(species: WildlifeSpecies): string {
  switch (species) {
    case "rabbit": return "R";
    case "deer": return "D";
    case "mallard": return "M";
    case "bee": return "B";
  }
}

function populationView(canvas: CanvasSize, state: TerrainLabState): {
  minX: number;
  minZ: number;
  width: number;
  height: number;
} {
  const width = state.blocksAcross;
  const height = width * canvas.cssHeight / Math.max(canvas.cssWidth, 1);
  return {
    minX: state.centerX - width * 0.5,
    minZ: state.centerZ - height * 0.5,
    width,
    height,
  };
}

function drawLoadingGrid(
  context: CanvasRenderingContext2D,
  canvas: CanvasSize,
): void {
  context.strokeStyle = "rgba(214, 235, 146, 0.08)";
  context.lineWidth = Math.max(1, canvas.dpr);
  const spacing = 64 * canvas.dpr;
  for (let x = 0; x < canvas.width; x += spacing) {
    context.beginPath();
    context.moveTo(x, 0);
    context.lineTo(x, canvas.height);
    context.stroke();
  }
  for (let y = 0; y < canvas.height; y += spacing) {
    context.beginPath();
    context.moveTo(0, y);
    context.lineTo(canvas.width, y);
    context.stroke();
  }
}

function reportFromSummary(
  summary: WildlifePopulationSummary,
  buildMs: number,
): WildlifePopulationReport {
  return {
    seed: summary.seed,
    schema: summary.schema,
    revision: summary.revision,
    buildMs,
    checksum: summary.checksum,
    cellCount: summary.cells.length,
    occupiedCells: summary.occupiedCells,
    animalCount: summary.animalCount,
    speciesCounts: summary.speciesCounts,
  };
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
