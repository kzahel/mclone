import { create } from "zustand";
import {
  parseAnimalCatalog,
  type AnimalCatalogDocument,
  type AnimalCatalogFigure,
} from "../catalog-model";

export type LoadStatus = "idle" | "loading" | "ready" | "error";
export type MotionFilter =
  | "all"
  | "quadruped-walk"
  | "biped-walk"
  | "wing-flap"
  | "swim"
  | "slither"
  | "other";
export type PromotionFilter = "all" | "runtime" | "asset-lab";
export type ThemeMode = "light" | "dark";

interface AnimalCatalogueState {
  catalog: AnimalCatalogDocument | null;
  error: string | null;
  loadCatalog: () => Promise<void>;
  loadStatus: LoadStatus;
  motionFilter: MotionFilter;
  promotionFilter: PromotionFilter;
  restoreUrlSelection: () => void;
  search: string;
  selectClip: (clipName: string) => void;
  selectFigure: (figureName: string) => void;
  selectedClipName: string | null;
  selectedFigureName: string | null;
  setMotionFilter: (filter: MotionFilter) => void;
  setPromotionFilter: (filter: PromotionFilter) => void;
  setSearch: (search: string) => void;
  syncSystemTheme: (themeMode: ThemeMode) => void;
  themeMode: ThemeMode;
  themeSource: "manual" | "system";
  toggleTheme: () => void;
}

export const useAnimalCatalogueStore = create<AnimalCatalogueState>((set, get) => ({
  catalog: null,
  error: null,
  loadStatus: "idle",
  motionFilter: "all",
  promotionFilter: "all",
  search: "",
  selectedClipName: null,
  selectedFigureName: null,
  themeMode: systemThemeMode(),
  themeSource: "system",

  async loadCatalog() {
    if (get().loadStatus === "loading") {
      return;
    }
    set({ error: null, loadStatus: "loading" });
    try {
      const response = await fetch(assetUrl("catalog/catalog.v1.json"), { cache: "no-cache" });
      if (!response.ok) {
        throw new Error(`Could not load animal catalogue: HTTP ${response.status}`);
      }
      const catalog = parseAnimalCatalog(await response.json(), "deployed catalog/catalog.v1.json");
      const selection = selectionFromUrl(catalog);
      set({
        catalog,
        error: null,
        loadStatus: "ready",
        selectedClipName: selection.clipName,
        selectedFigureName: selection.figure.name,
      });
      updateUrl(selection.figure.name, selection.clipName, "replace");
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : String(error),
        loadStatus: "error",
      });
    }
  },

  restoreUrlSelection() {
    const catalog = get().catalog;
    if (!catalog) {
      return;
    }
    const selection = selectionFromUrl(catalog);
    set({
      selectedClipName: selection.clipName,
      selectedFigureName: selection.figure.name,
    });
  },

  selectFigure(figureName) {
    const figure = get().catalog?.figures.find((entry) => entry.name === figureName);
    if (!figure) {
      return;
    }
    set({ selectedClipName: figure.defaultClip, selectedFigureName: figure.name });
    updateUrl(figure.name, figure.defaultClip, "push");
  },

  selectClip(clipName) {
    const state = get();
    const figure = state.catalog?.figures.find((entry) => entry.name === state.selectedFigureName);
    if (!figure?.clips.some((clip) => clip.name === clipName)) {
      return;
    }
    set({ selectedClipName: clipName });
    updateUrl(figure.name, clipName, "replace");
  },

  setSearch(search) {
    set({ search });
  },

  setMotionFilter(motionFilter) {
    set({ motionFilter });
  },

  setPromotionFilter(promotionFilter) {
    set({ promotionFilter });
  },

  syncSystemTheme(themeMode) {
    if (get().themeSource === "system") {
      set({ themeMode });
    }
  },

  toggleTheme() {
    set((state) => ({
      themeMode: state.themeMode === "dark" ? "light" : "dark",
      themeSource: "manual",
    }));
  },
}));

export function assetUrl(path: string): string {
  return `${import.meta.env.BASE_URL}${path}`;
}

function selectionFromUrl(catalog: AnimalCatalogDocument): {
  clipName: string;
  figure: AnimalCatalogFigure;
} {
  const params = new URLSearchParams(window.location.search);
  const requestedFigure = params.get("figure");
  const figure = catalog.figures.find((entry) => entry.name === requestedFigure)
    ?? catalog.figures.find((entry) => entry.name === "chicken")
    ?? catalog.figures[0];
  if (!figure) {
    throw new Error("The deployed animal catalogue is empty");
  }
  const requestedClip = params.get("clip");
  const clipName = figure.clips.some((clip) => clip.name === requestedClip)
    ? requestedClip as string
    : figure.defaultClip;
  return { clipName, figure };
}

function updateUrl(figureName: string, clipName: string, mode: "push" | "replace"): void {
  const url = new URL(window.location.href);
  url.searchParams.set("figure", figureName);
  url.searchParams.set("clip", clipName);
  if (mode === "push") {
    window.history.pushState(null, "", url);
  } else {
    window.history.replaceState(null, "", url);
  }
}

function systemThemeMode(): ThemeMode {
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}
