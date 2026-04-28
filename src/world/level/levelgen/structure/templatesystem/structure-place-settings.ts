import { BlockPos } from "../../../../../core/block-pos";
import { SimpleRandomSource } from "../../../../../worldgen/prng/simple-random-source";
import { Mirror } from "../../../block/mirror";
import { Rotation } from "../../../block/rotation";
import { BoundingBox } from "../bounding-box";
import type { StructureProcessor } from "./structure-processor";

export class StructurePlaceSettings {
  private mirror = Mirror.NONE;
  private rotation = Rotation.NONE;
  private rotationPivot = BlockPos.ZERO;
  private ignoreEntities = false;
  private boundingBox: BoundingBox | undefined = undefined;
  private keepLiquids = true;
  private random: SimpleRandomSource | undefined = undefined;
  private readonly processors: StructureProcessor[] = [];
  private knownShape = false;
  private finalizeEntities = false;

  public setMirror(mirror: Mirror): StructurePlaceSettings {
    this.mirror = mirror;
    return this;
  }

  public setRotation(rotation: Rotation): StructurePlaceSettings {
    this.rotation = rotation;
    return this;
  }

  public setRotationPivot(rotationPivot: BlockPos): StructurePlaceSettings {
    this.rotationPivot = rotationPivot;
    return this;
  }

  public setIgnoreEntities(ignoreEntities: boolean): StructurePlaceSettings {
    this.ignoreEntities = ignoreEntities;
    return this;
  }

  public setBoundingBox(boundingBox: BoundingBox): StructurePlaceSettings {
    this.boundingBox = boundingBox;
    return this;
  }

  public setRandom(random: SimpleRandomSource): StructurePlaceSettings {
    this.random = random;
    return this;
  }

  public setKeepLiquids(keepLiquids: boolean): StructurePlaceSettings {
    this.keepLiquids = keepLiquids;
    return this;
  }

  public setKnownShape(knownShape: boolean): StructurePlaceSettings {
    this.knownShape = knownShape;
    return this;
  }

  public clearProcessors(): StructurePlaceSettings {
    this.processors.length = 0;
    return this;
  }

  public addProcessor(processor: StructureProcessor): StructurePlaceSettings {
    this.processors.push(processor);
    return this;
  }

  public getMirror(): Mirror {
    return this.mirror;
  }

  public getRotation(): Rotation {
    return this.rotation;
  }

  public getRotationPivot(): BlockPos {
    return this.rotationPivot;
  }

  public getRandom(seedPos?: BlockPos): SimpleRandomSource {
    if (this.random !== undefined) {
      return this.random;
    }

    return seedPos === undefined ? new SimpleRandomSource(0n) : new SimpleRandomSource(seedPos.asLong());
  }

  public isIgnoreEntities(): boolean {
    return this.ignoreEntities;
  }

  public getBoundingBox(): BoundingBox | undefined {
    return this.boundingBox;
  }

  public getKnownShape(): boolean {
    return this.knownShape;
  }

  public getProcessors(): readonly StructureProcessor[] {
    return this.processors;
  }

  public shouldKeepLiquids(): boolean {
    return this.keepLiquids;
  }

  public setFinalizeEntities(finalizeEntities: boolean): StructurePlaceSettings {
    this.finalizeEntities = finalizeEntities;
    return this;
  }

  public shouldFinalizeEntities(): boolean {
    return this.finalizeEntities;
  }
}
