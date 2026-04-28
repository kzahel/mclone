import { ResourceLocation } from "../../core/resource-location";
import type { ClientEntityPresentationState } from "../../runtime/client/entity-interpolation-service";
import {
  CHICKEN_ENTITY_TYPE_ID,
  COW_ENTITY_TYPE_ID,
  MOOSHROOM_ENTITY_TYPE_ID,
  PIG_ENTITY_TYPE_ID,
  PLAYER_ENTITY_TYPE_ID,
  RABBIT_ENTITY_TYPE_ID,
  SHEEP_ENTITY_TYPE_ID,
  WOLF_ENTITY_TYPE_ID,
} from "../../runtime/protocol/world-messages";

export const DEFAULT_PLAYER_SKIN = new ResourceLocation("minecraft", "textures/entity/steve.png");
export const DEFAULT_CHICKEN_TEXTURE = new ResourceLocation("minecraft", "textures/entity/chicken.png");
export const DEFAULT_COW_TEXTURE = new ResourceLocation("minecraft", "textures/entity/cow/cow.png");
export const DEFAULT_BROWN_MOOSHROOM_TEXTURE = new ResourceLocation("minecraft", "textures/entity/cow/brown_mooshroom.png");
export const DEFAULT_RED_MOOSHROOM_TEXTURE = new ResourceLocation("minecraft", "textures/entity/cow/red_mooshroom.png");
export const DEFAULT_PIG_TEXTURE = new ResourceLocation("minecraft", "textures/entity/pig/pig.png");
export const DEFAULT_RABBIT_BLACK_TEXTURE = new ResourceLocation("minecraft", "textures/entity/rabbit/black.png");
export const DEFAULT_RABBIT_BROWN_TEXTURE = new ResourceLocation("minecraft", "textures/entity/rabbit/brown.png");
export const DEFAULT_RABBIT_EVIL_TEXTURE = new ResourceLocation("minecraft", "textures/entity/rabbit/caerbannog.png");
export const DEFAULT_RABBIT_GOLD_TEXTURE = new ResourceLocation("minecraft", "textures/entity/rabbit/gold.png");
export const DEFAULT_RABBIT_SALT_TEXTURE = new ResourceLocation("minecraft", "textures/entity/rabbit/salt.png");
export const DEFAULT_RABBIT_TOAST_TEXTURE = new ResourceLocation("minecraft", "textures/entity/rabbit/toast.png");
export const DEFAULT_RABBIT_WHITE_TEXTURE = new ResourceLocation("minecraft", "textures/entity/rabbit/white.png");
export const DEFAULT_RABBIT_WHITE_SPLOTCHED_TEXTURE = new ResourceLocation("minecraft", "textures/entity/rabbit/white_splotched.png");
export const DEFAULT_SHEEP_TEXTURE = new ResourceLocation("minecraft", "textures/entity/sheep/sheep.png");
export const DEFAULT_SHEEP_FUR_TEXTURE = new ResourceLocation("minecraft", "textures/entity/sheep/sheep_fur.png");
export const DEFAULT_WOLF_ANGRY_TEXTURE = new ResourceLocation("minecraft", "textures/entity/wolf/wolf_angry.png");
export const DEFAULT_WOLF_TEXTURE = new ResourceLocation("minecraft", "textures/entity/wolf/wolf.png");
export const DEFAULT_WOLF_TAME_TEXTURE = new ResourceLocation("minecraft", "textures/entity/wolf/wolf_tame.png");

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

export interface RenderableMooshroom extends RenderableTexturedMob {
  getMushroomType(): "red" | "brown";
}

export interface RenderablePig extends RenderableTexturedMob {}

export interface RenderableRabbit extends RenderableTexturedMob {
  getJumpCompletion(partialTick: number): number;
  getRabbitType(): number;
}

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

export interface RenderableWolf extends RenderableTexturedMob {
  getBodyRollAngle(partialTicks: number, offset: number): number;
  getHeadRollAngle(partialTicks: number): number;
  getTailAngle(): number;
  getWetShade(partialTicks: number): number;
  isAngry(): boolean;
  isInSittingPose(): boolean;
  isTame(): boolean;
  isWet(): boolean;
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
    const previousData = state.previousAuthoritative?.data;
    this.yBodyRot = readNumberData(state.data?.YBodyRot, this.yaw);
    this.yBodyRotO = readNumberData(
      state.data?.YBodyRotO,
      readNumberData(previousData?.YBodyRot, state.previousAuthoritative?.rotation.yaw ?? this.yBodyRot),
    );
    this.yHeadRot = readNumberData(state.data?.YHeadRot, this.yBodyRot);
    this.yHeadRotO = readNumberData(
      state.data?.YHeadRotO,
      readNumberData(previousData?.YHeadRot, this.yHeadRot),
    );
    this.xRotO = readNumberData(state.data?.XRotO, state.previousAuthoritative?.rotation.pitch ?? this.pitch);
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

export class SnapshotRenderableMooshroom extends SnapshotRenderableTexturedMob implements RenderableMooshroom {
  private readonly mushroomType: "red" | "brown";

  public constructor(state: ClientEntityPresentationState) {
    const mushroomType = readMooshroomType(state.data?.Type);
    super(state, mushroomType === "brown" ? DEFAULT_BROWN_MOOSHROOM_TEXTURE : DEFAULT_RED_MOOSHROOM_TEXTURE);
    this.mushroomType = mushroomType;
  }

  public getMushroomType(): "red" | "brown" {
    return this.mushroomType;
  }
}

export class SnapshotRenderablePig extends SnapshotRenderableTexturedMob implements RenderablePig {
  public constructor(state: ClientEntityPresentationState) {
    super(state, DEFAULT_PIG_TEXTURE);
  }
}

export class SnapshotRenderableRabbit extends SnapshotRenderableTexturedMob implements RenderableRabbit {
  private readonly rabbitType: number;
  private readonly jumpTicks: number;
  private readonly jumpDuration: number;

  public constructor(state: ClientEntityPresentationState) {
    const rabbitType = readIntegerData(state.data?.RabbitType, 0);
    super(state, getRabbitTextureLocation(rabbitType, state.data?.CustomName));
    this.rabbitType = rabbitType;
    this.jumpTicks = readIntegerData(state.data?.JumpTicks, 0);
    this.jumpDuration = readIntegerData(state.data?.JumpDuration, 0);
  }

  public getJumpCompletion(partialTick: number): number {
    return this.jumpDuration === 0 ? 0.0 : (this.jumpTicks + partialTick) / this.jumpDuration;
  }

  public getRabbitType(): number {
    return this.rabbitType;
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
  private readonly eatAnimationTick: number;

  public constructor(state: ClientEntityPresentationState) {
    super(state, DEFAULT_SHEEP_TEXTURE);
    this.color = readSheepColor(state.data?.Color);
    this.sheared = state.data?.Sheared === true;
    this.eatAnimationTick = Math.trunc(readNumberData(state.data?.EatAnimationTick, 0.0));
  }

  public getColor(): number {
    return this.color;
  }

  public isSheared(): boolean {
    return this.sheared;
  }

  public getHeadEatPositionScale(partialTick: number): number {
    if (this.eatAnimationTick <= 0) {
      return 0.0;
    }
    if (this.eatAnimationTick >= 4 && this.eatAnimationTick <= 36) {
      return 1.0;
    }
    return this.eatAnimationTick < 4
      ? (this.eatAnimationTick - partialTick) / 4.0
      : -(this.eatAnimationTick - 40 - partialTick) / 4.0;
  }

  public getHeadEatAngleScale(partialTick: number): number {
    if (this.eatAnimationTick > 4 && this.eatAnimationTick <= 36) {
      const phase = (this.eatAnimationTick - 4 - partialTick) / 32.0;
      return (Math.PI / 5.0) + (0.21991149 * Math.sin(phase * 28.7));
    }
    return this.eatAnimationTick > 0 ? Math.PI / 5.0 : this.getXRot() * (Math.PI / 180.0);
  }
}

export class SnapshotRenderableWolf extends SnapshotRenderableTexturedMob implements RenderableWolf {
  private readonly angry: boolean;
  private readonly bodyRoll: number;
  private readonly bodyRollO: number;
  private readonly headRoll: number;
  private readonly headRollO: number;
  private readonly maxHealth: number;
  private readonly health: number;
  private readonly sitting: boolean;
  private readonly tame: boolean;
  private readonly wet: boolean;

  public constructor(state: ClientEntityPresentationState) {
    const tame = state.data?.Tame === true;
    const angry = state.data?.Angry === true || readIntegerData(state.data?.RemainingAngerTime, 0) > 0;
    super(state, tame ? DEFAULT_WOLF_TAME_TEXTURE : (angry ? DEFAULT_WOLF_ANGRY_TEXTURE : DEFAULT_WOLF_TEXTURE));
    this.angry = angry;
    this.bodyRoll = readNumberData(state.data?.ShakeAnim, 0.0);
    this.bodyRollO = readNumberData(state.data?.ShakeAnimO, this.bodyRoll);
    this.headRoll = readNumberData(state.data?.InterestedAngle, 0.0);
    this.headRollO = readNumberData(state.data?.InterestedAngleO, this.headRoll);
    this.health = readNumberData(state.data?.Health, tame ? 20.0 : 8.0);
    this.maxHealth = readNumberData(state.data?.MaxHealth, tame ? 20.0 : 8.0);
    this.sitting = state.data?.Sitting === true;
    this.tame = tame;
    this.wet = state.data?.Wet === true;
  }

  public getBodyRollAngle(partialTicks: number, offset: number): number {
    let roll = (lerpNumber(partialTicks, this.bodyRollO, this.bodyRoll) + offset) / 1.8;
    roll = Math.max(0.0, Math.min(1.0, roll));
    return Math.sin(roll * Math.PI) * Math.sin(roll * Math.PI * 11.0) * 0.15 * Math.PI;
  }

  public getHeadRollAngle(partialTicks: number): number {
    return lerpNumber(partialTicks, this.headRollO, this.headRoll) * 0.15 * Math.PI;
  }

  public getTailAngle(): number {
    if (this.angry) {
      return 1.5393804;
    }
    return this.tame ? (0.55 - ((this.maxHealth - this.health) * 0.02)) * Math.PI : Math.PI / 5.0;
  }

  public getWetShade(partialTicks: number): number {
    return Math.min(0.5 + ((lerpNumber(partialTicks, this.bodyRollO, this.bodyRoll) / 2.0) * 0.5), 1.0);
  }

  public isAngry(): boolean {
    return this.angry;
  }

  public isInSittingPose(): boolean {
    return this.sitting;
  }

  public isTame(): boolean {
    return this.tame;
  }

  public isWet(): boolean {
    return this.wet;
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

  if (state.typeId === MOOSHROOM_ENTITY_TYPE_ID) {
    return new SnapshotRenderableMooshroom(state);
  }

  if (state.typeId === PIG_ENTITY_TYPE_ID) {
    return new SnapshotRenderablePig(state);
  }

  if (state.typeId === RABBIT_ENTITY_TYPE_ID) {
    return new SnapshotRenderableRabbit(state);
  }

  if (state.typeId === SHEEP_ENTITY_TYPE_ID) {
    return new SnapshotRenderableSheep(state);
  }

  if (state.typeId === WOLF_ENTITY_TYPE_ID) {
    return new SnapshotRenderableWolf(state);
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

function readMooshroomType(value: unknown): "red" | "brown" {
  return value === "brown" ? "brown" : "red";
}

function readNumberData(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function readIntegerData(value: unknown, fallback: number): number {
  return Math.trunc(readNumberData(value, fallback));
}

function getRabbitTextureLocation(rabbitType: number, customName: unknown): ResourceLocation {
  if (customName === "Toast") {
    return DEFAULT_RABBIT_TOAST_TEXTURE;
  }
  switch (rabbitType) {
    case 1:
      return DEFAULT_RABBIT_WHITE_TEXTURE;
    case 2:
      return DEFAULT_RABBIT_BLACK_TEXTURE;
    case 3:
      return DEFAULT_RABBIT_WHITE_SPLOTCHED_TEXTURE;
    case 4:
      return DEFAULT_RABBIT_GOLD_TEXTURE;
    case 5:
      return DEFAULT_RABBIT_SALT_TEXTURE;
    case 99:
      return DEFAULT_RABBIT_EVIL_TEXTURE;
    case 0:
    default:
      return DEFAULT_RABBIT_BROWN_TEXTURE;
  }
}

function lerpNumber(partial: number, start: number, end: number): number {
  return start + (partial * (end - start));
}
