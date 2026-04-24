import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { SectionPos } from "../../../core/section-pos";
import { LightLayer } from "../light-layer";
import { DataLayer } from "../chunk/data-layer";
import type { LightChunkGetter } from "../chunk/light-chunk-getter";
import { LayerLightEngine } from "./layer-light-engine";
import { SkyDataLayerStorageMap, SkyLightSectionStorage } from "./sky-light-section-storage";
import { LIGHT_SELF_SOURCE } from "./section-tracker";

const DIRECTIONS = Direction.values();
const HORIZONTALS = [Direction.NORTH, Direction.SOUTH, Direction.WEST, Direction.EAST] as const;

function signum(value: number): number {
  return value === 0 ? 0 : (value > 0 ? 1 : -1);
}

export class SkyLightEngine extends LayerLightEngine<SkyDataLayerStorageMap, SkyLightSectionStorage> {
  public constructor(chunkSource: LightChunkGetter) {
    super(chunkSource, LightLayer.SKY, new SkyLightSectionStorage(chunkSource));
  }

  protected override computeLevelFromNeighbor(source: bigint, target: bigint, sourceLevel: number): number {
    if (target === LIGHT_SELF_SOURCE || source === LIGHT_SELF_SOURCE) {
      return 15;
    }
    if (sourceLevel >= 15) {
      return sourceLevel;
    }

    const opacity = { value: 0 };
    const targetState = this.getStateAndOpacity(target, opacity);
    if (opacity.value >= 15) {
      return 15;
    }

    const sourceX = BlockPos.getX(source);
    const sourceY = BlockPos.getY(source);
    const sourceZ = BlockPos.getZ(source);
    const targetX = BlockPos.getX(target);
    const targetY = BlockPos.getY(target);
    const targetZ = BlockPos.getZ(target);
    const dx = signum(targetX - sourceX);
    const dy = signum(targetY - sourceY);
    const dz = signum(targetZ - sourceZ);
    const direction = Direction.fromNormal(dx, dy, dz);
    if (direction === undefined) {
      throw new Error(`Light was spread in illegal direction ${dx}, ${dy}, ${dz}`);
    }

    const sourceState = this.getStateAndOpacity(source, undefined);
    if (this.shapesFaceOcclude(sourceState, targetState, direction)) {
      return 15;
    }

    const sameColumn = sourceX === targetX && sourceZ === targetZ;
    const downward = sameColumn && sourceY > targetY;
    return downward && sourceLevel === 0 && opacity.value === 0 ? 0 : sourceLevel + Math.max(1, opacity.value);
  }

  protected override checkNeighborsAfterUpdate(pos: bigint, level: number, decrease: boolean): void {
    const section = SectionPos.blockToSection(pos);
    const y = BlockPos.getY(pos);
    const sectionRelativeY = SectionPos.sectionRelative(y);
    const sectionY = SectionPos.blockToSectionCoord(y);
    let skippedSections: number;
    if (sectionRelativeY !== 0) {
      skippedSections = 0;
    } else {
      let cursor = 0;
      while (
        !this.storage.storingLightForSection(SectionPos.offset(section, 0, -cursor - 1, 0)) &&
        this.storage.hasSectionsBelow(sectionY - cursor - 1)
      ) {
        cursor++;
      }

      skippedSections = cursor;
    }

    const down = BlockPos.offset(pos, 0, -1 - (skippedSections * 16), 0);
    const downSection = SectionPos.blockToSection(down);
    if (section === downSection || this.storage.storingLightForSection(downSection)) {
      this.checkNeighbor(pos, down, level, decrease);
    }

    const up = BlockPos.offset(pos, Direction.UP);
    const upSection = SectionPos.blockToSection(up);
    if (section === upSection || this.storage.storingLightForSection(upSection)) {
      this.checkNeighbor(pos, up, level, decrease);
    }

    for (const direction of HORIZONTALS) {
      let verticalOffset = 0;
      do {
        const horizontal = BlockPos.offset(pos, direction.getStepX(), -verticalOffset, direction.getStepZ());
        const horizontalSection = SectionPos.blockToSection(horizontal);
        if (section === horizontalSection) {
          this.checkNeighbor(pos, horizontal, level, decrease);
          break;
        }

        if (this.storage.storingLightForSection(horizontalSection)) {
          const verticalSource = BlockPos.offset(pos, 0, -verticalOffset, 0);
          this.checkNeighbor(verticalSource, horizontal, level, decrease);
        }

        verticalOffset++;
      } while (verticalOffset <= skippedSections * 16);
    }
  }

  protected override getComputedLevel(pos: bigint, source: bigint, candidateLevel: number): number {
    let level = candidateLevel;
    const section = SectionPos.blockToSection(pos);
    const dataLayer = this.storage.getDataLayer(section, true);

    for (const direction of DIRECTIONS) {
      const neighbor = BlockPos.offset(pos, direction);
      if (neighbor !== source) {
        const neighborSection = SectionPos.blockToSection(neighbor);
        const neighborDataLayer: DataLayer | undefined = section === neighborSection
          ? dataLayer
          : this.storage.getDataLayer(neighborSection, true);

        let neighborLevel: number;
        if (neighborDataLayer !== undefined) {
          neighborLevel = this.getLevelFromDataLayer(neighborDataLayer, neighbor);
        } else {
          if (direction === Direction.DOWN) {
            continue;
          }

          neighborLevel = 15 - this.storage.getLightValueFrom(neighbor, true);
        }

        const computedLevel = this.computeLevelFromNeighbor(neighbor, pos, neighborLevel);
        if (level > computedLevel) {
          level = computedLevel;
        }

        if (level === 0) {
          return level;
        }
      }
    }

    return level;
  }

  protected override checkNode(pos: bigint): void {
    this.storage.runAllUpdates();
    let section = SectionPos.blockToSection(pos);
    if (this.storage.storingLightForSection(section)) {
      super.checkNode(pos);
      return;
    }

    for (
      pos = BlockPos.getFlatIndex(pos);
      !this.storage.storingLightForSection(section) && !this.storage.isAboveData(section);
      pos = BlockPos.offset(pos, 0, 16, 0)
    ) {
      section = SectionPos.offset(section, Direction.UP);
    }

    if (this.storage.storingLightForSection(section)) {
      super.checkNode(pos);
    }
  }

  public override getDebugData(section: bigint): string {
    return `${super.getDebugData(section)}${this.storage.isAboveData(section) ? "*" : ""}`;
  }
}
