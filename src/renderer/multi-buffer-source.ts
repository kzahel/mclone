import { BufferBuilder } from "./vertex/buffer-builder";
import { RenderType } from "./render-type";
import type { VertexConsumer } from "./vertex/vertex-consumer";

export interface MultiBufferSource {
  getBuffer(renderType: RenderType): VertexConsumer;
}

export namespace MultiBufferSource {
  export function immediate(builder: BufferBuilder): BufferSource {
    return immediateWithBuffers(new Map(), builder);
  }

  export function immediateWithBuffers(mapBuilders: ReadonlyMap<RenderType, BufferBuilder>, builder: BufferBuilder): BufferSource {
    return new BufferSource(builder, mapBuilders);
  }

  export class BufferSource implements MultiBufferSource {
    protected readonly builder: BufferBuilder;
    protected readonly fixedBuffers: ReadonlyMap<RenderType, BufferBuilder>;
    protected lastState: RenderType | undefined;
    protected readonly startedBuffers = new Set<BufferBuilder>();

    public constructor(builder: BufferBuilder, fixedBuffers: ReadonlyMap<RenderType, BufferBuilder>) {
      this.builder = builder;
      this.fixedBuffers = fixedBuffers;
    }

    public getBuffer(renderType: RenderType): VertexConsumer {
      const optionalRenderType = renderType.asOptional();
      const builder = this.getBuilderRaw(renderType);
      if (this.lastState !== optionalRenderType) {
        if (this.lastState !== undefined && !this.fixedBuffers.has(this.lastState)) {
          this.endBatch(this.lastState);
        }

        if (!this.startedBuffers.has(builder)) {
          this.startedBuffers.add(builder);
          builder.begin(renderType.mode(), renderType.format());
        }

        this.lastState = optionalRenderType;
      }

      return builder;
    }

    private getBuilderRaw(renderType: RenderType): BufferBuilder {
      return this.fixedBuffers.get(renderType) ?? this.builder;
    }

    public endLastBatch(): void {
      if (this.lastState !== undefined) {
        const renderType = this.lastState;
        if (!this.fixedBuffers.has(renderType)) {
          this.endBatch(renderType);
        }

        this.lastState = undefined;
      }
    }

    public endBatch(): void;
    public endBatch(renderType: RenderType): void;
    public endBatch(renderType?: RenderType): void {
      if (renderType !== undefined) {
        this.endBatchForRenderType(renderType);
        return;
      }

      if (this.lastState !== undefined) {
        const vertexConsumer = this.getBuffer(this.lastState);
        if (vertexConsumer === this.builder) {
          this.endBatchForRenderType(this.lastState);
        }
      }

      for (const fixedRenderType of this.fixedBuffers.keys()) {
        this.endBatchForRenderType(fixedRenderType);
      }
    }

    private endBatchForRenderType(renderType: RenderType): void {
      const builder = this.getBuilderRaw(renderType);
      const isLastState = this.lastState === renderType.asOptional();
      if (isLastState || builder !== this.builder) {
        if (this.startedBuffers.delete(builder)) {
          renderType.end(builder, 0, 0, 0);
          if (isLastState) {
            this.lastState = undefined;
          }
        }
      }
    }
  }
}
