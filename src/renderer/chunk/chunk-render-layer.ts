import { RenderType } from "../render-type";

export const CHUNK_RENDER_LAYERS = [
  RenderType.solid(),
  RenderType.cutoutMipped(),
  RenderType.cutout(),
  RenderType.translucent(),
  RenderType.tripwire(),
] as const;

export type ChunkRenderLayer = (typeof CHUNK_RENDER_LAYERS)[number];
export type ChunkRenderLayerName = "solid" | "cutout_mipped" | "cutout" | "translucent" | "tripwire";

const CHUNK_RENDER_LAYER_BY_NAME = new Map<string, ChunkRenderLayer>(
  CHUNK_RENDER_LAYERS.map((renderType) => [renderType.name, renderType] as const),
);

export function getChunkRenderLayerByName(name: string): ChunkRenderLayer {
  const renderType = CHUNK_RENDER_LAYER_BY_NAME.get(name);
  if (renderType === undefined) {
    throw new Error(`Unknown chunk render layer ${name}`);
  }

  return renderType;
}

export function getChunkRenderLayerName(renderType: RenderType): ChunkRenderLayerName {
  const layer = CHUNK_RENDER_LAYER_BY_NAME.get(renderType.name);
  if (layer === undefined) {
    throw new Error(`RenderType ${renderType.name} is not a chunk render layer`);
  }

  return layer.name as ChunkRenderLayerName;
}
