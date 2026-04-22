import { BufferBuilder } from "./vertex/buffer-builder";
import { RenderType } from "./render-type";

export class ChunkBufferBuilderPack {
  private readonly builders = new Map<RenderType, BufferBuilder>(
    RenderType.chunkBufferLayers().map((renderType) => [renderType, new BufferBuilder(renderType.bufferSize())] as const),
  );

  public builder(renderType: RenderType): BufferBuilder {
    return this.builders.get(renderType)!;
  }

  public clearAll(): void {
    for (const builder of this.builders.values()) {
      builder.clear();
    }
  }

  public discardAll(): void {
    for (const builder of this.builders.values()) {
      builder.discard();
    }
  }
}
