import { create } from "zustand";
import type { TextureCandidateEntry, TextureImageRef, TextureIndexEntry, TextureLabIndex } from "../../core/index-model";

type LoadStatus = "idle" | "loading" | "ready" | "error";
export type ThemeMode = "light" | "dark";
export type PreviewMode = "detail" | "atlas" | "blocks";
export type QueueFilter = "all" | "noise-placeholder" | "authored-structure" | "frozen-asset" | "has-candidates" | "needs-candidates";
type ThemeSource = "system" | "manual";

export interface TextureLabState {
  index: TextureLabIndex | null;
  selectedTextureName: string | null;
  selectedCandidateId: string | null;
  previewSelectionsByTexture: Record<string, string>;
  themeMode: ThemeMode;
  themeSource: ThemeSource;
  previewMode: PreviewMode;
  search: string;
  materialFilter: string;
  statusFilter: string;
  queueFilter: QueueFilter;
  loadStatus: LoadStatus;
  error: string | null;
  curationStatus: string | null;
  loadIndex: () => Promise<void>;
  reindex: () => Promise<void>;
  selectTexture: (name: string) => void;
  selectCandidate: (id: string) => void;
  setPreviewCandidate: (textureName: string, candidateId: string) => void;
  clearPreviewCandidate: (textureName: string) => void;
  selectCurationCandidate: (textureName: string, candidateId: string) => Promise<void>;
  clearCurationSelection: (textureName: string) => Promise<void>;
  applyCuration: () => Promise<void>;
  requestFreeze: (textureName: string) => Promise<void>;
  setPreviewMode: (mode: PreviewMode) => void;
  syncSystemTheme: (themeMode: ThemeMode) => void;
  toggleTheme: () => void;
  setSearch: (search: string) => void;
  setMaterialFilter: (material: string) => void;
  setStatusFilter: (status: string) => void;
  setQueueFilter: (queueFilter: QueueFilter) => void;
}

export const useTextureLabStore = create<TextureLabState>((set, get) => ({
  index: null,
  selectedTextureName: null,
  selectedCandidateId: null,
  previewSelectionsByTexture: {},
  themeMode: systemThemeMode(),
  themeSource: "system",
  previewMode: "detail",
  search: "",
  materialFilter: "all",
  statusFilter: "all",
  queueFilter: "all",
  loadStatus: "idle",
  error: null,
  curationStatus: null,

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
        previewSelectionsByTexture: prunePreviewSelections(index, get().previewSelectionsByTexture),
        loadStatus: "ready",
        error: null,
        curationStatus: null,
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
        previewSelectionsByTexture: prunePreviewSelections(index, get().previewSelectionsByTexture),
        loadStatus: "ready",
        error: null,
        curationStatus: null,
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

  setPreviewCandidate(textureName, candidateId) {
    const index = get().index;
    const candidate = index?.candidates.find((entry) => entry.id === candidateId);
    if (!candidate || candidate.textureName !== textureName) {
      return;
    }
    set((state) => ({
      previewSelectionsByTexture: {
        ...state.previewSelectionsByTexture,
        [textureName]: candidateId,
      },
    }));
  },

  clearPreviewCandidate(textureName) {
    set((state) => {
      const { [textureName]: _removed, ...previewSelectionsByTexture } = state.previewSelectionsByTexture;
      return { previewSelectionsByTexture };
    });
  },

  async selectCurationCandidate(textureName, candidateId) {
    set({ loadStatus: "loading", error: null, curationStatus: null });
    try {
      const index = await postIndex("/api/curation/select", { textureName, candidateId });
      set({
        index,
        selectedTextureName: selectTextureAfterLoad(index, get().selectedTextureName),
        selectedCandidateId: selectCandidateAfterLoad(index, textureName, candidateId),
        previewSelectionsByTexture: prunePreviewSelections(index, get().previewSelectionsByTexture),
        loadStatus: "ready",
        error: null,
        curationStatus: `Selected candidate for ${textureName}`,
      });
    } catch (error) {
      set({ loadStatus: "error", error: error instanceof Error ? error.message : String(error) });
    }
  },

  async clearCurationSelection(textureName) {
    set({ loadStatus: "loading", error: null, curationStatus: null });
    try {
      const index = await postIndex("/api/curation/clear", { textureName });
      set({
        index,
        selectedTextureName: selectTextureAfterLoad(index, get().selectedTextureName),
        selectedCandidateId: selectCandidateAfterLoad(index, textureName, get().selectedCandidateId),
        previewSelectionsByTexture: prunePreviewSelections(index, get().previewSelectionsByTexture),
        loadStatus: "ready",
        error: null,
        curationStatus: `Cleared pack selection for ${textureName}`,
      });
    } catch (error) {
      set({ loadStatus: "error", error: error instanceof Error ? error.message : String(error) });
    }
  },

  async applyCuration() {
    set({ loadStatus: "loading", error: null, curationStatus: null });
    try {
      const response = await postApplyCuration();
      set({
        index: response.index,
        selectedTextureName: selectTextureAfterLoad(response.index, get().selectedTextureName),
        selectedCandidateId: selectCandidateAfterLoad(response.index, get().selectedTextureName, get().selectedCandidateId),
        previewSelectionsByTexture: prunePreviewSelections(response.index, get().previewSelectionsByTexture),
        loadStatus: "ready",
        error: null,
        curationStatus: `Applied ${response.result.applied.length} selection${response.result.applied.length === 1 ? "" : "s"} to generated pack`,
      });
    } catch (error) {
      set({ loadStatus: "error", error: error instanceof Error ? error.message : String(error) });
    }
  },

  async requestFreeze(textureName) {
    set({ loadStatus: "loading", error: null, curationStatus: null });
    try {
      const response = await postFreezeRequest(textureName);
      set({
        loadStatus: "ready",
        error: null,
        curationStatus: `Freeze request written: ${response.request.path}`,
      });
    } catch (error) {
      set({ loadStatus: "error", error: error instanceof Error ? error.message : String(error) });
    }
  },

  setPreviewMode(previewMode) {
    set({ previewMode });
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

  setQueueFilter(queueFilter) {
    set({ queueFilter });
  },
}));

async function fetchIndex(url: string, init?: RequestInit): Promise<TextureLabIndex> {
  return fetchJson<TextureLabIndex>(url, init);
}

async function postIndex(url: string, body: unknown): Promise<TextureLabIndex> {
  return fetchIndex(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
}

async function postApplyCuration(): Promise<{
  result: { applied: { textureName: string }[] };
  index: TextureLabIndex;
}> {
  return fetchJson("/api/curation/apply", { method: "POST" });
}

async function postFreezeRequest(textureName: string): Promise<{
  request: { path: string };
}> {
  return fetchJson("/api/freeze-request", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ textureName }),
  });
}

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, init);
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: string } | null;
    throw new Error(body?.error ?? `Texture-lab API returned ${response.status}`);
  }
  return (await response.json()) as T;
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

function prunePreviewSelections(
  index: TextureLabIndex,
  current: Record<string, string>,
): Record<string, string> {
  const pruned: Record<string, string> = {};
  for (const [textureName, candidateId] of Object.entries(current)) {
    const candidate = index.candidates.find((entry) => entry.id === candidateId);
    if (candidate?.textureName === textureName) {
      pruned[textureName] = candidateId;
    }
  }
  return pruned;
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

export function tintedImageRefUrl(ref: TextureImageRef, tint: string): string | null {
  return ref.path && ref.exists
    ? `/api/tinted-image?path=${encodeURIComponent(ref.path)}&tint=${encodeURIComponent(tint)}`
    : null;
}
