import { create } from "zustand";
import {
  parseStructureCatalog,
  type StructureCatalogDocument,
  type StructureCatalogEntry,
} from "../catalog-model";
import type { CameraPreset } from "../viewport";

export type LoadStatus = "idle" | "loading" | "ready" | "error";
export type ThemeMode = "light" | "dark";
export type StatusFilter = "all" | "promoted" | "parity-canary" | "lab-only";
export type StructureRotation = 0 | 90 | 180 | 270;

interface StructureLabState {
  boundsVisible: boolean;
  cameraPreset: CameraPreset;
  catalog: StructureCatalogDocument | null;
  error: string | null;
  familyFilter: string;
  hiddenComponents: string[];
  layer: number;
  loadCatalog: () => Promise<void>;
  loadStatus: LoadStatus;
  markersVisible: boolean;
  mirror: boolean;
  restoreUrlSelection: () => void;
  rotation: StructureRotation;
  search: string;
  selectedStructureId: string | null;
  selectStructure: (id: string) => void;
  setBoundsVisible: (visible: boolean) => void;
  setCameraPreset: (preset: CameraPreset) => void;
  setFamilyFilter: (family: string) => void;
  setLayer: (layer: number) => void;
  setMarkersVisible: (visible: boolean) => void;
  setMirror: (mirror: boolean) => void;
  setRotation: (rotation: StructureRotation) => void;
  setSearch: (search: string) => void;
  setStatusFilter: (filter: StatusFilter) => void;
  statusFilter: StatusFilter;
  syncSystemTheme: (theme: ThemeMode) => void;
  themeMode: ThemeMode;
  themeSource: "manual" | "system";
  toggleComponent: (id: string) => void;
  toggleTheme: () => void;
}

const cameraPresets = new Set<CameraPreset>(["three-quarter", "front", "side", "top"]);
const rotations = new Set<StructureRotation>([0, 90, 180, 270]);

export const useStructureLabStore = create<StructureLabState>((set, get) => ({
  boundsVisible: false,
  cameraPreset: "three-quarter",
  catalog: null,
  error: null,
  familyFilter: "all",
  hiddenComponents: [],
  layer: 0,
  loadStatus: "idle",
  markersVisible: true,
  mirror: false,
  rotation: 0,
  search: "",
  selectedStructureId: null,
  statusFilter: "all",
  themeMode: systemThemeMode(),
  themeSource: "system",

  async loadCatalog() {
    if (get().loadStatus === "loading") return;
    set({ error: null, loadStatus: "loading" });
    try {
      const response = await fetch(assetUrl("catalog/catalog.v1.json"), { cache: "no-cache" });
      if (!response.ok) throw new Error(`Could not load Structure Lab: HTTP ${response.status}`);
      const catalog = parseStructureCatalog(await response.json(), "deployed Structure Lab catalog");
      const selection = selectionFromUrl(catalog);
      set({ catalog, error: null, loadStatus: "ready", ...selection });
      updateUrl(get(), "replace");
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error), loadStatus: "error" });
    }
  },

  restoreUrlSelection() {
    const catalog = get().catalog;
    if (catalog) set(selectionFromUrl(catalog));
  },

  selectStructure(id) {
    const structure = get().catalog?.structures.find((entry) => entry.structureId === id);
    if (!structure) return;
    set({
      selectedStructureId: id,
      hiddenComponents: [],
      layer: structure.size[1] - 1,
    });
    updateUrl(get(), "push");
  },

  setBoundsVisible(boundsVisible) {
    set({ boundsVisible });
    updateUrl(get(), "replace");
  },
  setCameraPreset(cameraPreset) {
    set({ cameraPreset });
    updateUrl(get(), "replace");
  },
  setFamilyFilter(familyFilter) { set({ familyFilter }); },
  setLayer(layer) {
    set({ layer });
    updateUrl(get(), "replace");
  },
  setMarkersVisible(markersVisible) {
    set({ markersVisible });
    updateUrl(get(), "replace");
  },
  setMirror(mirror) {
    set({ mirror });
    updateUrl(get(), "replace");
  },
  setRotation(rotation) {
    set({ rotation });
    updateUrl(get(), "replace");
  },
  setSearch(search) { set({ search }); },
  setStatusFilter(statusFilter) { set({ statusFilter }); },
  syncSystemTheme(themeMode) {
    if (get().themeSource === "system") set({ themeMode });
  },
  toggleComponent(id) {
    set((state) => ({
      hiddenComponents: state.hiddenComponents.includes(id)
        ? state.hiddenComponents.filter((entry) => entry !== id)
        : [...state.hiddenComponents, id].sort(),
    }));
    updateUrl(get(), "replace");
  },
  toggleTheme() {
    set((state) => ({
      themeMode: state.themeMode === "dark" ? "light" : "dark",
      themeSource: "manual",
    }));
  },
}));

export function assetUrl(relativePath: string): string {
  return `${import.meta.env.BASE_URL}${relativePath}`;
}

function selectionFromUrl(catalog: StructureCatalogDocument): Pick<
  StructureLabState,
  | "boundsVisible"
  | "cameraPreset"
  | "hiddenComponents"
  | "layer"
  | "markersVisible"
  | "mirror"
  | "rotation"
  | "selectedStructureId"
> {
  const params = new URLSearchParams(window.location.search);
  const requested = params.get("structure");
  const structure = catalog.structures.find((entry) => entry.structureId === requested)
    ?? catalog.structures[0];
  if (!structure) throw new Error("The deployed Structure Lab catalog is empty");
  const cameraValue = params.get("camera") as CameraPreset | null;
  const rotationValue = Number(params.get("rotation"));
  const layerParam = params.get("layer");
  const requestedLayer = layerParam === null ? Number.NaN : Number(layerParam);
  const componentIds = new Set(structure.components.map((component) => component.id));
  return {
    boundsVisible: params.get("bounds") === "1",
    cameraPreset: cameraValue !== null && cameraPresets.has(cameraValue)
      ? cameraValue
      : "three-quarter",
    hiddenComponents: (params.get("hide") ?? "")
      .split(",")
      .filter((id) => componentIds.has(id))
      .sort(),
    layer: Number.isInteger(requestedLayer)
      ? Math.max(0, Math.min(structure.size[1] - 1, requestedLayer))
      : structure.size[1] - 1,
    markersVisible: params.get("markers") !== "0",
    mirror: params.get("mirror") === "1",
    rotation: rotations.has(rotationValue as StructureRotation)
      ? rotationValue as StructureRotation
      : 0,
    selectedStructureId: structure.structureId,
  };
}

function updateUrl(state: StructureLabState, mode: "push" | "replace"): void {
  if (!state.selectedStructureId) return;
  const url = new URL(window.location.href);
  url.searchParams.set("structure", state.selectedStructureId);
  url.searchParams.set("camera", state.cameraPreset);
  url.searchParams.set("layer", String(state.layer));
  setOptionalParam(url, "hide", state.hiddenComponents.join(","));
  setOptionalParam(url, "rotation", state.rotation === 0 ? "" : String(state.rotation));
  setOptionalParam(url, "mirror", state.mirror ? "1" : "");
  setOptionalParam(url, "bounds", state.boundsVisible ? "1" : "");
  setOptionalParam(url, "markers", state.markersVisible ? "" : "0");
  window.history[mode === "push" ? "pushState" : "replaceState"](null, "", url);
}

function setOptionalParam(url: URL, key: string, value: string): void {
  if (value === "") url.searchParams.delete(key);
  else url.searchParams.set(key, value);
}

function systemThemeMode(): ThemeMode {
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function selectedStructure(
  catalog: StructureCatalogDocument | null,
  id: string | null,
): StructureCatalogEntry | undefined {
  return catalog?.structures.find((entry) => entry.structureId === id);
}
