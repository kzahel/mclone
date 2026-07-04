import type { TextureCandidateEntry, TextureIndexEntry, TextureLabIndex } from "../../core/index-model";
import type { TextureLabState } from "./textureLabStore";

export const selectIndex = (state: TextureLabState): TextureLabIndex | null => state.index;
export const selectLoadStatus = (state: TextureLabState): TextureLabState["loadStatus"] => state.loadStatus;
export const selectError = (state: TextureLabState): string | null => state.error;
export const selectCurationStatus = (state: TextureLabState): string | null => state.curationStatus;
export const selectSelectedTextureName = (state: TextureLabState): string | null => state.selectedTextureName;
export const selectSelectedCandidateId = (state: TextureLabState): string | null => state.selectedCandidateId;
export const selectPreviewSelectionsByTexture = (state: TextureLabState): Record<string, string> =>
  state.previewSelectionsByTexture;
export const selectThemeMode = (state: TextureLabState): TextureLabState["themeMode"] => state.themeMode;
export const selectPreviewMode = (state: TextureLabState): TextureLabState["previewMode"] => state.previewMode;
export const selectSearch = (state: TextureLabState): string => state.search;
export const selectMaterialFilter = (state: TextureLabState): string => state.materialFilter;
export const selectStatusFilter = (state: TextureLabState): string => state.statusFilter;

export function selectedTexture(state: TextureLabState): TextureIndexEntry | null {
  if (!state.index || !state.selectedTextureName) {
    return null;
  }
  return state.index.textures.find((texture) => texture.name === state.selectedTextureName) ?? null;
}

export function activeTextureCandidates(state: TextureLabState): TextureCandidateEntry[] {
  if (!state.index || !state.selectedTextureName) {
    return [];
  }
  return state.index.candidates.filter((candidate) => candidate.textureName === state.selectedTextureName);
}

export function selectedCandidate(state: TextureLabState): TextureCandidateEntry | null {
  if (!state.index || !state.selectedCandidateId) {
    return null;
  }
  return state.index.candidates.find((candidate) => candidate.id === state.selectedCandidateId) ?? null;
}

export function filteredTextures(state: TextureLabState): TextureIndexEntry[] {
  if (!state.index) {
    return [];
  }
  const search = state.search.trim().toLowerCase();
  return state.index.textures.filter((texture) => {
    if (state.materialFilter !== "all" && texture.materialFamily !== state.materialFilter) {
      return false;
    }
    if (state.statusFilter !== "all" && texture.status !== state.statusFilter) {
      return false;
    }
    if (!search) {
      return true;
    }
    return [
      texture.name,
      texture.displayName,
      texture.materialFamily,
      texture.palette,
      texture.artSource.kind,
      texture.artSource.label,
      texture.artSource.description,
      texture.tintRole ?? "",
      ...texture.tags,
      ...texture.blockUsages.map((usage) => usage.blockName),
    ]
      .join(" ")
      .toLowerCase()
      .includes(search);
  });
}

export function materialOptions(state: TextureLabState): string[] {
  return uniqueSorted(state.index?.textures.map((texture) => texture.materialFamily) ?? []);
}

export function statusOptions(state: TextureLabState): string[] {
  return uniqueSorted(state.index?.textures.map((texture) => texture.status) ?? []);
}

function uniqueSorted(values: string[]): string[] {
  return [...new Set(values)].sort((left, right) => left.localeCompare(right));
}
