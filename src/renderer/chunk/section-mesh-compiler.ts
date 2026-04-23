import { BlockPos } from "../../core/block-pos";
import { JavaRandom } from "../../util/java-random";
import type { BlockState } from "../../world/level/block/state/block-state";
import { RenderShape } from "../../world/level/block/render-shape";
import { Vec3 } from "../../world/phys/vec3";
import { BlockRenderDispatcher } from "../block/block-render-dispatcher";
import { ModelBlockRenderer } from "../block/model-block-renderer";
import { ChunkBufferBuilderPack } from "../chunk-buffer-builder-pack";
import { ItemBlockRenderTypes } from "../item-block-render-types";
import { RenderType } from "../render-type";
import { BufferBuilder, BufferBuilderSortState } from "../vertex/buffer-builder";
import { DefaultVertexFormat } from "../vertex/default-vertex-format";
import { PoseStack } from "../vertex/pose-stack";
import { VertexFormat } from "../vertex/vertex-format";
import { RenderChunkRegion } from "./render-chunk-region";
import { VisGraph } from "./vis-graph";
import { VisibilitySet } from "./visibility-set";

export interface SectionMeshBuild {
  readonly hasBlocks: ReadonlySet<RenderType>;
  readonly hasLayers: ReadonlySet<RenderType>;
  readonly isCompletelyEmpty: boolean;
  readonly visibilitySet: VisibilitySet;
  readonly transparencyState: BufferBuilderSortState | undefined;
  readonly globalBlockEntities: ReadonlySet<unknown>;
}

export function beginChunkRenderLayer(builder: BufferBuilder): void {
  builder.begin(VertexFormat.Mode.QUADS, DefaultVertexFormat.BLOCK);
}

export function buildSectionMesh(
  origin: BlockPos,
  camera: Vec3,
  region: RenderChunkRegion | null,
  blockRenderer: BlockRenderDispatcher,
  buffers: ChunkBufferBuilderPack,
): SectionMeshBuild {
  const end = origin.offset(15, 15, 15);
  const visibilityGraph = new VisGraph();
  const globalBlockEntities = new Set<unknown>();
  const poseStack = new PoseStack();
  const hasBlocks = new Set<RenderType>();
  const hasLayers = new Set<RenderType>();
  let isCompletelyEmpty = true;
  let transparencyState: BufferBuilderSortState | undefined;

  if (region !== null) {
    ModelBlockRenderer.enableCaching();
    try {
      const random = new JavaRandom();
      for (let z = origin.getZ(); z <= end.getZ(); z++) {
        for (let y = origin.getY(); y <= end.getY(); y++) {
          for (let x = origin.getX(); x <= end.getX(); x++) {
            const pos = new BlockPos(x, y, z);
            const state: BlockState = region.getBlockState(pos);
            const fluidState = state.getFluidState();
            if (state.isSolidRender(region, pos)) {
              visibilityGraph.setOpaque(pos);
            }

            if (!fluidState.isEmpty()) {
              const renderType = ItemBlockRenderTypes.getRenderLayer(fluidState);
              const builder = buffers.builder(renderType);
              if (!hasLayers.has(renderType)) {
                hasLayers.add(renderType);
                beginChunkRenderLayer(builder);
              }

              if (blockRenderer.renderLiquid(pos, region, builder, fluidState)) {
                isCompletelyEmpty = false;
                hasBlocks.add(renderType);
              }
            }

            if (state.getRenderShape() === RenderShape.INVISIBLE) {
              continue;
            }

            const renderType = ItemBlockRenderTypes.getChunkRenderType(state);
            const builder = buffers.builder(renderType);
            if (!hasLayers.has(renderType)) {
              hasLayers.add(renderType);
              beginChunkRenderLayer(builder);
            }

            poseStack.pushPose();
            poseStack.translate(pos.getX() & 15, pos.getY() & 15, pos.getZ() & 15);
            if (blockRenderer.renderBatched(state, pos, region, poseStack, builder, true, random)) {
              isCompletelyEmpty = false;
              hasBlocks.add(renderType);
            }
            poseStack.popPose();
          }
        }
      }

      if (hasBlocks.has(RenderType.translucent())) {
        const translucentBuilder = buffers.builder(RenderType.translucent());
        translucentBuilder.setQuadSortOrigin(
          camera.x - origin.getX(),
          camera.y - origin.getY(),
          camera.z - origin.getZ(),
        );
        transparencyState = translucentBuilder.getSortState();
      }

      for (const renderType of hasLayers) {
        buffers.builder(renderType).end();
      }
    } finally {
      ModelBlockRenderer.clearCache();
    }
  }

  return {
    hasBlocks,
    hasLayers,
    isCompletelyEmpty,
    visibilitySet: visibilityGraph.resolve(),
    transparencyState,
    globalBlockEntities,
  };
}
