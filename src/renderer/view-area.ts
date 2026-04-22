import { BlockPos } from "../core/block-pos";
import { floor, intFloorDiv, positiveModulo } from "../util/mth";
import { StaticRenderLevel } from "../world/level/static-render-level";
import { ChunkRenderDispatcher } from "./chunk/chunk-render-dispatcher";
import { LevelRenderer } from "./level-renderer";

export class ViewArea {
  protected readonly chunkGridSizeY: number;
  protected readonly chunkGridSizeX: number;
  protected readonly chunkGridSizeZ: number;
  public readonly chunks: ChunkRenderDispatcher.RenderChunk[];

  public constructor(
    protected readonly chunkRenderDispatcher: ChunkRenderDispatcher,
    protected readonly level: StaticRenderLevel,
    viewDistance: number,
    protected readonly levelRenderer: LevelRenderer,
  ) {
    const size = viewDistance * 2 + 1;
    this.chunkGridSizeX = size;
    this.chunkGridSizeY = this.level.getSectionsCount();
    this.chunkGridSizeZ = size;
    this.chunks = new Array<ChunkRenderDispatcher.RenderChunk>(this.chunkGridSizeX * this.chunkGridSizeY * this.chunkGridSizeZ);
    this.createChunks();
  }

  protected createChunks(): void {
    for (let x = 0; x < this.chunkGridSizeX; x++) {
      for (let y = 0; y < this.chunkGridSizeY; y++) {
        for (let z = 0; z < this.chunkGridSizeZ; z++) {
          const index = this.getChunkIndex(x, y, z);
          this.chunks[index] = this.chunkRenderDispatcher.createRenderChunk(index);
          this.chunks[index]!.setOrigin(x * 16, y * 16, z * 16);
        }
      }
    }
  }

  public releaseAllBuffers(): void {
    for (const chunk of this.chunks) {
      chunk.releaseBuffers();
    }
  }

  private getChunkIndex(x: number, y: number, z: number): number {
    return ((z * this.chunkGridSizeY) + y) * this.chunkGridSizeX + x;
  }

  public repositionCamera(cameraX: number, cameraZ: number): void {
    const floorX = floor(cameraX);
    const floorZ = floor(cameraZ);

    for (let gridX = 0; gridX < this.chunkGridSizeX; gridX++) {
      const width = this.chunkGridSizeX * 16;
      const minX = floorX - 8 - Math.trunc(width / 2);
      const originX = minX + positiveModulo((gridX * 16) - minX, width);

      for (let gridZ = 0; gridZ < this.chunkGridSizeZ; gridZ++) {
        const depth = this.chunkGridSizeZ * 16;
        const minZ = floorZ - 8 - Math.trunc(depth / 2);
        const originZ = minZ + positiveModulo((gridZ * 16) - minZ, depth);

        for (let gridY = 0; gridY < this.chunkGridSizeY; gridY++) {
          const originY = this.level.getMinBuildHeight() + (gridY * 16);
          this.chunks[this.getChunkIndex(gridX, gridY, gridZ)]!.setOrigin(originX, originY, originZ);
        }
      }
    }
  }

  public setDirty(chunkX: number, sectionY: number, chunkZ: number, playerChanged: boolean): void {
    const wrappedX = positiveModulo(chunkX, this.chunkGridSizeX);
    const wrappedY = positiveModulo(sectionY - this.level.getMinSection(), this.chunkGridSizeY);
    const wrappedZ = positiveModulo(chunkZ, this.chunkGridSizeZ);
    this.chunks[this.getChunkIndex(wrappedX, wrappedY, wrappedZ)]!.setDirty(playerChanged);
  }

  public getRenderChunkAt(pos: BlockPos): ChunkRenderDispatcher.RenderChunk | null {
    let chunkX = intFloorDiv(pos.getX(), 16);
    const sectionY = intFloorDiv(pos.getY() - this.level.getMinBuildHeight(), 16);
    let chunkZ = intFloorDiv(pos.getZ(), 16);
    if (sectionY < 0 || sectionY >= this.chunkGridSizeY) {
      return null;
    }

    chunkX = positiveModulo(chunkX, this.chunkGridSizeX);
    chunkZ = positiveModulo(chunkZ, this.chunkGridSizeZ);
    return this.chunks[this.getChunkIndex(chunkX, sectionY, chunkZ)]!;
  }
}
