import { ResourceLocation } from "../../core/resource-location";
import type { ClientEntityPresentationState } from "../../runtime/client/entity-interpolation-service";
import {
  CHICKEN_ENTITY_TYPE_ID,
  COW_ENTITY_TYPE_ID,
  PIG_ENTITY_TYPE_ID,
  PLAYER_ENTITY_TYPE_ID,
  SHEEP_ENTITY_TYPE_ID,
} from "../../runtime/protocol/world-messages";

export const DEFAULT_PLAYER_SKIN = new ResourceLocation("minecraft", "textures/entity/steve.png");
export const DEFAULT_CHICKEN_TEXTURE = new ResourceLocation("minecraft", "textures/entity/chicken.png");
export const DEFAULT_COW_TEXTURE = new ResourceLocation("minecraft", "textures/entity/cow/cow.png");
export const DEFAULT_PIG_TEXTURE = new ResourceLocation("minecraft", "textures/entity/pig/pig.png");
export const DEFAULT_SHEEP_TEXTURE = new ResourceLocation("minecraft", "textures/entity/sheep/sheep.png");
export const DEFAULT_SHEEP_FUR_TEXTURE = new ResourceLocation("minecraft", "textures/entity/sheep/sheep_fur.png");

export type PlayerSkinModel = "default" | "slim";

export interface RenderableEntity {
  readonly entityId: number;
  readonly uuid: string;
  readonly typeId: string;
  readonly tickCount: number;
  readonly yBodyRot: number;
  readonly yBodyRotO: number;
  readonly yHeadRot: number;
  readonly yHeadRotO: number;
  readonly xRotO: number;
  getX(): number;
  getY(): number;
  getZ(): number;
  getXRot(): number;
  getYRot(): number;
  getBbWidth(): number;
  getBbHeight(): number;
  isInvisible(): boolean;
  isSpectator(): boolean;
  isPassenger(): boolean;
  isAlive(): boolean;
  isBaby(): boolean;
}

export interface RenderablePlayer extends RenderableEntity {
  getModelName(): PlayerSkinModel;
  getSkinTextureLocation(): ResourceLocation;
  isCrouching(): boolean;
  hasChestEquipment(): boolean;
}

export interface RenderableTexturedMob extends RenderableEntity {
  getTextureLocation(): ResourceLocation;
}

export interface RenderableCow extends RenderableTexturedMob {}

export interface RenderablePig extends RenderableTexturedMob {}

export interface RenderableChicken extends RenderableTexturedMob {
  getFlap(): number;
  getFlapSpeed(): number;
  getOFlap(): number;
  getOFlapSpeed(): number;
  getFlapping(): number;
  getEggLayTime(): number;
  isChickenJockey(): boolean;
}

export interface RenderableSheep extends RenderableTexturedMob {
  getColor(): number;
  isSheared(): boolean;
  getHeadEatPositionScale(partialTick: number): number;
  getHeadEatAngleScale(partialTick: number): number;
}

export class SnapshotRenderablePlayer implements RenderablePlayer {
  public readonly entityId: number;
  public readonly uuid: string;
  public readonly typeId = PLAYER_ENTITY_TYPE_ID;
  public readonly tickCount: number;
  public readonly yBodyRot: number;
  public readonly yBodyRotO: number;
  public readonly yHeadRot: number;
  public readonly yHeadRotO: number;
  public readonly xRotO: number;

  private readonly x: number;
  private readonly y: number;
  private readonly z: number;
  private readonly yaw: number;
  private readonly pitch: number;
  private readonly width: number;
  private readonly height: number;
  private readonly skinModel: PlayerSkinModel;
  private readonly skinTextureLocation: ResourceLocation;
  private readonly crouching: boolean;

  public constructor(state: ClientEntityPresentationState) {
    this.entityId = state.entityId;
    this.uuid = state.uuid;
    this.x = state.interpolatedPosition.x;
    this.y = state.interpolatedPosition.y;
    this.z = state.interpolatedPosition.z;
    this.yaw = state.interpolatedRotation.yaw;
    this.pitch = state.interpolatedRotation.pitch;
    this.width = state.width;
    this.height = state.height;
    this.tickCount = state.age ?? 0;
    this.yBodyRot = this.yaw;
    this.yBodyRotO = state.previousAuthoritative?.rotation.yaw ?? this.yaw;
    this.yHeadRot = this.yaw;
    this.yHeadRotO = this.yBodyRotO;
    this.xRotO = state.previousAuthoritative?.rotation.pitch ?? this.pitch;
    this.skinModel = readSkinModel(state.data?.skinModel);
    this.skinTextureLocation = readSkinTextureLocation(state.data?.skinTexture);
    this.crouching = state.data?.crouching === true;
  }

  public getX(): number {
    return this.x;
  }

  public getY(): number {
    return this.y;
  }

  public getZ(): number {
    return this.z;
  }

  public getXRot(): number {
    return this.pitch;
  }

  public getYRot(): number {
    return this.yaw;
  }

  public getBbWidth(): number {
    return this.width;
  }

  public getBbHeight(): number {
    return this.height;
  }

  public isInvisible(): boolean {
    return false;
  }

  public isSpectator(): boolean {
    return false;
  }

  public isPassenger(): boolean {
    return false;
  }

  public isAlive(): boolean {
    return true;
  }

  public isBaby(): boolean {
    return false;
  }

  public getModelName(): PlayerSkinModel {
    return this.skinModel;
  }

  public getSkinTextureLocation(): ResourceLocation {
    return this.skinTextureLocation;
  }

  public isCrouching(): boolean {
    return this.crouching;
  }

  public hasChestEquipment(): boolean {
    return false;
  }
}

abstract class SnapshotRenderableTexturedMob implements RenderableTexturedMob {
  public readonly entityId: number;
  public readonly uuid: string;
  public readonly typeId: string;
  public readonly tickCount: number;
  public readonly yBodyRot: number;
  public readonly yBodyRotO: number;
  public readonly yHeadRot: number;
  public readonly yHeadRotO: number;
  public readonly xRotO: number;

  private readonly x: number;
  private readonly y: number;
  private readonly z: number;
  private readonly yaw: number;
  private readonly pitch: number;
  private readonly width: number;
  private readonly height: number;
  private readonly baby: boolean;

  protected constructor(
    state: ClientEntityPresentationState,
    private readonly textureLocation: ResourceLocation,
  ) {
    this.entityId = state.entityId;
    this.uuid = state.uuid;
    this.typeId = state.typeId;
    this.x = state.interpolatedPosition.x;
    this.y = state.interpolatedPosition.y;
    this.z = state.interpolatedPosition.z;
    this.yaw = state.interpolatedRotation.yaw;
    this.pitch = state.interpolatedRotation.pitch;
    this.width = state.width;
    this.height = state.height;
    this.tickCount = state.age ?? 0;
    this.yBodyRot = this.yaw;
    this.yBodyRotO = state.previousAuthoritative?.rotation.yaw ?? this.yaw;
    this.yHeadRot = this.yaw;
    this.yHeadRotO = this.yBodyRotO;
    this.xRotO = state.previousAuthoritative?.rotation.pitch ?? this.pitch;
    this.baby = typeof state.age === "number" && state.age < 0;
  }

  public getX(): number {
    return this.x;
  }

  public getY(): number {
    return this.y;
  }

  public getZ(): number {
    return this.z;
  }

  public getXRot(): number {
    return this.pitch;
  }

  public getYRot(): number {
    return this.yaw;
  }

  public getBbWidth(): number {
    return this.width;
  }

  public getBbHeight(): number {
    return this.height;
  }

  public isInvisible(): boolean {
    return false;
  }

  public isSpectator(): boolean {
    return false;
  }

  public isPassenger(): boolean {
    return false;
  }

  public isAlive(): boolean {
    return true;
  }

  public isBaby(): boolean {
    return this.baby;
  }

  public getTextureLocation(): ResourceLocation {
    return this.textureLocation;
  }
}

export class SnapshotRenderableCow extends SnapshotRenderableTexturedMob implements RenderableCow {
  public constructor(state: ClientEntityPresentationState) {
    super(state, DEFAULT_COW_TEXTURE);
  }
}

export class SnapshotRenderablePig extends SnapshotRenderableTexturedMob implements RenderablePig {
  public constructor(state: ClientEntityPresentationState) {
    super(state, DEFAULT_PIG_TEXTURE);
  }
}

export class SnapshotRenderableChicken extends SnapshotRenderableTexturedMob implements RenderableChicken {
  private readonly flap: number;
  private readonly flapSpeed: number;
  private readonly oFlap: number;
  private readonly oFlapSpeed: number;
  private readonly flapping: number;
  private readonly eggLayTime: number;
  private readonly chickenJockey: boolean;

  public constructor(state: ClientEntityPresentationState) {
    super(state, DEFAULT_CHICKEN_TEXTURE);
    this.flap = readNumberData(state.data?.Flap, 0.0);
    this.flapSpeed = readNumberData(state.data?.FlapSpeed, 0.0);
    this.oFlap = readNumberData(state.data?.OFlap, 0.0);
    this.oFlapSpeed = readNumberData(state.data?.OFlapSpeed, 0.0);
    this.flapping = readNumberData(state.data?.Flapping, 1.0);
    this.eggLayTime = Math.trunc(readNumberData(state.data?.EggLayTime, 0.0));
    this.chickenJockey = state.data?.IsChickenJockey === true;
  }

  public getFlap(): number {
    return this.flap;
  }

  public getFlapSpeed(): number {
    return this.flapSpeed;
  }

  public getOFlap(): number {
    return this.oFlap;
  }

  public getOFlapSpeed(): number {
    return this.oFlapSpeed;
  }

  public getFlapping(): number {
    return this.flapping;
  }

  public getEggLayTime(): number {
    return this.eggLayTime;
  }

  public isChickenJockey(): boolean {
    return this.chickenJockey;
  }
}

export class SnapshotRenderableSheep extends SnapshotRenderableTexturedMob implements RenderableSheep {
  private readonly color: number;
  private readonly sheared: boolean;

  public constructor(state: ClientEntityPresentationState) {
    super(state, DEFAULT_SHEEP_TEXTURE);
    this.color = readSheepColor(state.data?.Color);
    this.sheared = state.data?.Sheared === true;
  }

  public getColor(): number {
    return this.color;
  }

  public isSheared(): boolean {
    return this.sheared;
  }

  public getHeadEatPositionScale(_partialTick: number): number {
    return 0.0;
  }

  public getHeadEatAngleScale(_partialTick: number): number {
    return this.getXRot() * (Math.PI / 180.0);
  }
}

export function createRenderableEntity(state: ClientEntityPresentationState): RenderableEntity | undefined {
  if (state.typeId === PLAYER_ENTITY_TYPE_ID) {
    return new SnapshotRenderablePlayer(state);
  }

  if (state.typeId === CHICKEN_ENTITY_TYPE_ID) {
    return new SnapshotRenderableChicken(state);
  }

  if (state.typeId === COW_ENTITY_TYPE_ID) {
    return new SnapshotRenderableCow(state);
  }

  if (state.typeId === PIG_ENTITY_TYPE_ID) {
    return new SnapshotRenderablePig(state);
  }

  if (state.typeId === SHEEP_ENTITY_TYPE_ID) {
    return new SnapshotRenderableSheep(state);
  }

  return undefined;
}

export function createRenderableEntities(states: readonly ClientEntityPresentationState[]): readonly RenderableEntity[] {
  return states.flatMap((state) => {
    const entity = createRenderableEntity(state);
    return entity === undefined ? [] : [entity];
  });
}

function readSkinModel(value: unknown): PlayerSkinModel {
  return value === "slim" ? "slim" : "default";
}

function readSkinTextureLocation(value: unknown): ResourceLocation {
  if (typeof value !== "string") {
    return DEFAULT_PLAYER_SKIN;
  }

  try {
    return new ResourceLocation(value);
  } catch {
    return DEFAULT_PLAYER_SKIN;
  }
}

function readSheepColor(value: unknown): number {
  return typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= 15 ? value : 0;
}

function readNumberData(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}
