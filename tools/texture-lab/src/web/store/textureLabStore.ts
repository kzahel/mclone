import { create } from "zustand";
import type { TextureIndexEntry, TextureLabIndex } from "../../core/index-model";

type LoadStatus = "idle" | "loading" | "ready" | "error";

export interface TextureLabState {
  index: TextureLabIndex | null;
  selectedTextureName: string | null;
  search: string;
  materialFilter: string;
  statusFilter: string;
  loadStatus: LoadStatus;
  error: string | null;
  loadIndex: () => Promise<void>;
  reindex: () => Promise<void>;
  selectTexture: (name: string) => void;
  setSearch: (search: string) => void;
  setMaterialFilter: (material: string) => void;
  setStatusFilter: (status: string) => void;
}

export const useTextureLabStore = create<TextureLabState>((set, get) => ({
  index: null,
  selectedTextureName: null,
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
      set({
        index,
        selectedTextureName: selectTextureAfterLoad(index, get().selectedTextureName),
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
      set({
        index,
        selectedTextureName: selectTextureAfterLoad(index, get().selectedTextureName),
        loadStatus: "ready",
        error: null,
      });
    } catch (error) {
      set({ loadStatus: "error", error: error instanceof Error ? error.message : String(error) });
    }
  },

  selectTexture(name) {
    set({ selectedTextureName: name });
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

export function imageUrl(texture: TextureIndexEntry, imageKind: keyof TextureIndexEntry["images"]): string | null {
  const ref = texture.images[imageKind];
  return ref.path && ref.exists ? `/api/image?path=${encodeURIComponent(ref.path)}` : null;
}
