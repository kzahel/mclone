import { create } from "zustand";
import type { TextureCandidateEntry, TextureImageRef, TextureIndexEntry, TextureLabIndex } from "../../core/index-model";

type LoadStatus = "idle" | "loading" | "ready" | "error";
export type ThemeMode = "light" | "dark";
type ThemeSource = "system" | "manual";

export interface TextureLabState {
  index: TextureLabIndex | null;
  selectedTextureName: string | null;
  selectedCandidateId: string | null;
  themeMode: ThemeMode;
  themeSource: ThemeSource;
  search: string;
  materialFilter: string;
  statusFilter: string;
  loadStatus: LoadStatus;
  error: string | null;
  loadIndex: () => Promise<void>;
  reindex: () => Promise<void>;
  selectTexture: (name: string) => void;
  selectCandidate: (id: string) => void;
  syncSystemTheme: (themeMode: ThemeMode) => void;
  toggleTheme: () => void;
  setSearch: (search: string) => void;
  setMaterialFilter: (material: string) => void;
  setStatusFilter: (status: string) => void;
}

export const useTextureLabStore = create<TextureLabState>((set, get) => ({
  index: null,
  selectedTextureName: null,
  selectedCandidateId: null,
  themeMode: systemThemeMode(),
  themeSource: "system",
  search: "",
  materialFilter: "all",
  statusFilter: "all",
  loadStatus: "idle",
  error: null,

  async loadIndex() {
    if (get().loadStatus === "loading") {
      return;
    }
    set({ loadStatus: "loading", error: null });
    try {
      const index = await fetchIndex("/api/index");
      const selectedTextureName = selectTextureAfterLoad(index, get().selectedTextureName);
      set({
        index,
        selectedTextureName,
        selectedCandidateId: selectCandidateAfterLoad(index, selectedTextureName, get().selectedCandidateId),
        loadStatus: "ready",
        error: null,
      });
    } catch (error) {
      set({ loadStatus: "error", error: error instanceof Error ? error.message : String(error) });
    }
  },

  async reindex() {
    set({ loadStatus: "loading", error: null });
    try {
      const index = await fetchIndex("/api/reindex", { method: "POST" });
      const selectedTextureName = selectTextureAfterLoad(index, get().selectedTextureName);
      set({
        index,
        selectedTextureName,
        selectedCandidateId: selectCandidateAfterLoad(index, selectedTextureName, get().selectedCandidateId),
        loadStatus: "ready",
        error: null,
      });
    } catch (error) {
      set({ loadStatus: "error", error: error instanceof Error ? error.message : String(error) });
    }
  },

  selectTexture(name) {
    const index = get().index;
    set({
      selectedTextureName: name,
      selectedCandidateId: index ? firstCandidateForTexture(index, name)?.id ?? null : null,
    });
  },

  selectCandidate(id) {
    set({ selectedCandidateId: id });
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

  setSearch(search) {
    set({ search });
  },

  setMaterialFilter(materialFilter) {
    set({ materialFilter });
  },

  setStatusFilter(statusFilter) {
    set({ statusFilter });
  },
}));

async function fetchIndex(url: string, init?: RequestInit): Promise<TextureLabIndex> {
  const response = await fetch(url, init);
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: string } | null;
    throw new Error(body?.error ?? `Texture-lab API returned ${response.status}`);
  }
  return (await response.json()) as TextureLabIndex;
}

function selectTextureAfterLoad(index: TextureLabIndex, current: string | null): string | null {
  if (current && index.textures.some((texture) => texture.name === current)) {
    return current;
  }
  return index.textures[0]?.name ?? null;
}

function selectCandidateAfterLoad(
  index: TextureLabIndex,
  selectedTextureName: string | null,
  currentCandidateId: string | null,
): string | null {
  if (!selectedTextureName) {
    return null;
  }
  if (currentCandidateId) {
    const current = index.candidates.find((candidate) => candidate.id === currentCandidateId);
    if (current?.textureName === selectedTextureName) {
      return current.id;
    }
  }
  return firstCandidateForTexture(index, selectedTextureName)?.id ?? null;
}

function firstCandidateForTexture(index: TextureLabIndex, textureName: string): TextureCandidateEntry | null {
  return index.candidates.find((candidate) => candidate.textureName === textureName) ?? null;
}

function systemThemeMode(): ThemeMode {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return "light";
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function imageUrl(texture: TextureIndexEntry, imageKind: keyof TextureIndexEntry["images"]): string | null {
  return imageRefUrl(texture.images[imageKind]);
}

export function imageRefUrl(ref: TextureImageRef): string | null {
  return ref.path && ref.exists ? `/api/image?path=${encodeURIComponent(ref.path)}` : null;
}
