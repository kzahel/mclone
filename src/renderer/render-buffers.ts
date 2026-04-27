import { ChunkBufferBuilderPack } from "./chunk-buffer-builder-pack";
import { MultiBufferSource } from "./multi-buffer-source";
import { RenderType } from "./render-type";
import { BufferBuilder } from "./vertex/buffer-builder";

export class RenderBuffers {
  private readonly fixedBufferPackValue = new ChunkBufferBuilderPack();
  private readonly fixedBuffersValue = new Map<RenderType, BufferBuilder>([
    [RenderType.solid(), this.fixedBufferPackValue.builder(RenderType.solid())],
    [RenderType.cutoutMipped(), this.fixedBufferPackValue.builder(RenderType.cutoutMipped())],
    [RenderType.cutout(), this.fixedBufferPackValue.builder(RenderType.cutout())],
    [RenderType.translucent(), this.fixedBufferPackValue.builder(RenderType.translucent())],
    [RenderType.tripwire(), this.fixedBufferPackValue.builder(RenderType.tripwire())],
    [RenderType.translucentNoCrumbling(), new BufferBuilder(RenderType.translucentNoCrumbling().bufferSize())],
  ]);
  private readonly bufferSourceValue = MultiBufferSource.immediateWithBuffers(this.fixedBuffersValue, new BufferBuilder(256));
  private readonly crumblingBufferSourceValue = MultiBufferSource.immediate(new BufferBuilder(256));

  public fixedBufferPack(): ChunkBufferBuilderPack {
    return this.fixedBufferPackValue;
  }

  public bufferSource(): MultiBufferSource.BufferSource {
    return this.bufferSourceValue;
  }

  public crumblingBufferSource(): MultiBufferSource.BufferSource {
    return this.crumblingBufferSourceValue;
  }
}
