import { SyntheticRuntimeEntity, type SyntheticRuntimeEntityOptions } from "../level/entity/entity-access";
import { AABB } from "../phys/aabb";
import { BlockPos } from "../../core/block-pos";
import { DEFAULT_MOB_ATTRIBUTES, MobAttribute, type MobAttribute as MobAttributeValue } from "./attribute";
import { GoalSelector } from "./ai/goal/goal-selector";
import { LookAtPlayerGoal } from "./ai/goal/look-at-player-goal";
import { RandomLookAroundGoal } from "./ai/goal/random-look-around-goal";
import { WaterAvoidingRandomStrollGoal } from "./ai/goal/water-avoiding-random-stroll-goal";
import { LookControl } from "./ai/control/look-control";
import { MoveControl } from "./ai/control/move-control";
import { GroundPathNavigation } from "./ai/navigation/ground-path-navigation";
import type { MobAiLevel, MobRandom, PathfinderMob } from "./ai/pathfinder-mob";
import { MobCategory } from "./mob-category";
import { WorldgenRandom } from "../../worldgen/prng/worldgen-random";
import { clamp, floor } from "../../util/mth";
import type { Fluid } from "../level/material/fluid";
import { BlockPathTypes, type BlockPathTypes as BlockPathType } from "../level/pathfinder/block-path-types";

interface GeneratedChickenRuntimeData {
  flap: number;
  flapSpeed: number;
  oFlap: number;
  oFlapSpeed: number;
  flapping: number;
  eggLayTime: number;
  isChickenJockey: boolean;
}

export interface GeneratedMobEntityOptions extends SyntheticRuntimeEntityOptions {
  readonly entityType: EntityType;
  readonly age?: number;
  readonly onGround?: boolean;
  readonly data?: Readonly<Record<string, number | boolean | string>>;
  readonly randomSeed?: bigint | number | string;
}

export interface GeneratedMobTickOptions {
  readonly resetNoActionTime?: boolean;
}

export class GeneratedMobEntity extends SyntheticRuntimeEntity implements PathfinderMob {
  public readonly entityType: EntityType;
  public readonly goalSelector = new GoalSelector();
  public readonly navigation: GroundPathNavigation;
  public readonly moveControl: MoveControl;
  public readonly lookControl: LookControl;
  public readonly age: number;
  public onGround: boolean;
  public readonly data: Readonly<Record<string, number | boolean | string>>;
  public tickCount = 0;
  private readonly random: WorldgenRandom;
  private aiLevel: MobAiLevel | undefined;
  private noActionTime = 0;
  private speed = 0.0;
  private zza = 0.0;
  private restrictCenter = BlockPos.ZERO;
  private restrictRadius = -1.0;
  private readonly pathfindingMalus = new Map<BlockPathType, number>();
  private yBodyRot = 0.0;
  private yBodyRotO = 0.0;
  private yHeadRot = 0.0;
  private yHeadRotO = 0.0;
  private xRotO = 0.0;
  private chickenRuntimeData: GeneratedChickenRuntimeData | undefined;

  public constructor(options: GeneratedMobEntityOptions) {
    super({
      ...options,
      typeId: options.entityType.id,
      width: options.entityType.width,
      height: options.entityType.height,
    });
    this.entityType = options.entityType;
    this.navigation = new GroundPathNavigation(this);
    this.moveControl = new MoveControl(this);
    this.lookControl = new LookControl(this);
    this.age = options.age ?? 0;
    this.onGround = options.onGround ?? false;
    this.yBodyRot = this.rotation.yaw;
    this.yBodyRotO = this.rotation.yaw;
    this.yHeadRot = this.rotation.yaw;
    this.yHeadRotO = this.rotation.yaw;
    this.xRotO = this.rotation.pitch;
    this.random = new WorldgenRandom(options.randomSeed ?? generatedEntityRandomSeed(options.id, options.uuid));
    this.data = options.data ?? this.entityType.createDefaultData(this.random);
    if (this.entityType.category === MobCategory.CREATURE) {
      this.setPathfindingMalus(BlockPathTypes.DANGER_FIRE, 16.0);
      this.setPathfindingMalus(BlockPathTypes.DAMAGE_FIRE, -1.0);
    }
    if (this.entityType.id === EntityTypes.CHICKEN.id) {
      this.setPathfindingMalus(BlockPathTypes.WATER, 0.0);
      this.chickenRuntimeData = createChickenRuntimeData(this.data, this.random);
    }
    this.registerGoals();
  }

  public setAiLevel(level: MobAiLevel | undefined): void {
    this.aiLevel = level;
  }

  public getAiLevel(): MobAiLevel | undefined {
    return this.aiLevel;
  }

  public tickServerAi(options: GeneratedMobTickOptions = {}): void {
    this.tickCount++;
    this.capturePreviousRotations();
    if (options.resetNoActionTime) {
      this.noActionTime = 0;
    }

    this.noActionTime++;
    this.goalSelector.tick();
    this.navigation.tick();
    this.moveControl.tick();
    this.lookControl.tick();
    this.applyControlledTravel();
    this.customAiStep();
  }

  public getX(): number {
    return this.position.x;
  }

  public getY(): number {
    return this.position.y;
  }

  public getZ(): number {
    return this.position.z;
  }

  public getEyeY(): number {
    return this.position.y + this.entityType.eyeHeight;
  }

  public getBlockY(): number {
    return floor(this.position.y);
  }

  public getYRot(): number {
    return this.rotation.yaw;
  }

  public setYRot(yaw: number): void {
    this.setRotation(yaw, this.rotation.pitch);
    this.setYBodyRot(yaw);
  }

  public getXRot(): number {
    return this.rotation.pitch;
  }

  public setXRot(pitch: number): void {
    this.setRotation(this.rotation.yaw, pitch);
  }

  public getYHeadRot(): number {
    return this.yHeadRot;
  }

  public setYHeadRot(yaw: number): void {
    this.yHeadRot = yaw;
  }

  public getYBodyRot(): number {
    return this.yBodyRot;
  }

  public setYBodyRot(yaw: number): void {
    this.yBodyRot = yaw;
  }

  public getMaxHeadXRot(): number {
    return 40;
  }

  public getMaxHeadYRot(): number {
    return 75;
  }

  public getHeadRotSpeed(): number {
    return 10;
  }

  public getBbWidth(): number {
    return this.entityType.width;
  }

  public getBbHeight(): number {
    return this.entityType.height;
  }

  public getMaxUpStep(): number {
    return 0.6;
  }

  public getMaxFallDistance(): number {
    return 3;
  }

  public getRandom(): MobRandom {
    return this.random;
  }

  public getNoActionTime(): number {
    return this.noActionTime;
  }

  public getNavigation(): GroundPathNavigation {
    return this.navigation;
  }

  public getMoveControl(): MoveControl {
    return this.moveControl;
  }

  public getLookControl(): LookControl {
    return this.lookControl;
  }

  public getSnapshotData(): Readonly<Record<string, number | boolean | string>> {
    const data = this.chickenRuntimeData === undefined
      ? this.data
      : {
        ...this.data,
        ...createChickenRuntimeDataSnapshot(this.chickenRuntimeData),
      };
    return {
      ...data,
      YBodyRot: this.yBodyRot,
      YBodyRotO: this.yBodyRotO,
      YHeadRot: this.yHeadRot,
      YHeadRotO: this.yHeadRotO,
      XRotO: this.xRotO,
    };
  }

  public getAttributeValue(attribute: MobAttributeValue): number {
    return this.entityType.getAttributeValue(attribute);
  }

  public getPathfindingMalus(type: BlockPathType): number {
    return this.pathfindingMalus.get(type) ?? type.getMalus();
  }

  public setPathfindingMalus(type: BlockPathType, priority: number): void {
    this.pathfindingMalus.set(type, priority);
  }

  public canCutCorner(type: BlockPathType): boolean {
    return type !== BlockPathTypes.DANGER_FIRE
      && type !== BlockPathTypes.DANGER_CACTUS
      && type !== BlockPathTypes.DANGER_OTHER
      && type !== BlockPathTypes.WALKABLE_DOOR;
  }

  public setSpeed(speed: number): void {
    this.speed = speed;
  }

  public setZza(forward: number): void {
    this.zza = forward;
  }

  public setXxa(_strafe: number): void {
  }

  public isVehicle(): boolean {
    return false;
  }

  public isAlive(): boolean {
    return this.removalReason === undefined;
  }

  public canStandOnFluid(_fluid: Fluid): boolean {
    return false;
  }

  public isOnGround(): boolean {
    return this.onGround;
  }

  public isInWaterOrBubble(): boolean {
    return this.aiLevel?.isWater(this.blockPosition()) ?? false;
  }

  public hasRestriction(): boolean {
    return this.restrictRadius !== -1.0;
  }

  public getRestrictCenter(): BlockPos {
    return this.restrictCenter;
  }

  public getRestrictRadius(): number {
    return this.restrictRadius;
  }

  public isWithinRestriction(pos: BlockPos): boolean {
    if (!this.hasRestriction()) {
      return true;
    }

    const dx = this.restrictCenter.getX() - pos.getX();
    const dy = this.restrictCenter.getY() - pos.getY();
    const dz = this.restrictCenter.getZ() - pos.getZ();
    return (dx * dx) + (dy * dy) + (dz * dz) < this.restrictRadius * this.restrictRadius;
  }

  public getWalkTargetValue(_pos: BlockPos): number {
    return 0.0;
  }

  public hasPathfindingMalus(_pos: BlockPos): boolean {
    return false;
  }

  public resetNoActionTime(): void {
    this.noActionTime = 0;
  }

  private registerGoals(): void {
    if (this.entityType.id === EntityTypes.COW.id) {
      this.goalSelector.addGoal(5, new WaterAvoidingRandomStrollGoal(this, 1.0));
      this.goalSelector.addGoal(6, new LookAtPlayerGoal(this, 6.0));
      this.goalSelector.addGoal(7, new RandomLookAroundGoal(this));
      return;
    }
    if (this.entityType.id === EntityTypes.CHICKEN.id) {
      this.goalSelector.addGoal(5, new WaterAvoidingRandomStrollGoal(this, 1.0));
      this.goalSelector.addGoal(6, new LookAtPlayerGoal(this, 6.0));
      this.goalSelector.addGoal(7, new RandomLookAroundGoal(this));
      return;
    }
    if (this.entityType.id === EntityTypes.PIG.id) {
      this.goalSelector.addGoal(6, new WaterAvoidingRandomStrollGoal(this, 1.0));
      this.goalSelector.addGoal(7, new LookAtPlayerGoal(this, 6.0));
      this.goalSelector.addGoal(8, new RandomLookAroundGoal(this));
      return;
    }
    if (this.entityType.id === EntityTypes.SHEEP.id) {
      this.goalSelector.addGoal(6, new WaterAvoidingRandomStrollGoal(this, 1.0));
      this.goalSelector.addGoal(7, new LookAtPlayerGoal(this, 6.0));
      this.goalSelector.addGoal(8, new RandomLookAroundGoal(this));
    }
  }

  private capturePreviousRotations(): void {
    this.yBodyRotO = this.yBodyRot;
    this.yHeadRotO = this.yHeadRot;
    this.xRotO = this.rotation.pitch;
  }

  private customAiStep(): void {
    if (this.entityType.id === EntityTypes.CHICKEN.id) {
      this.tickChickenAiStep();
    }
  }

  private tickChickenAiStep(): void {
    const data = this.chickenRuntimeData;
    if (data === undefined) {
      return;
    }

    data.oFlap = data.flap;
    data.oFlapSpeed = data.flapSpeed;
    data.flapSpeed = clamp(data.flapSpeed + ((this.onGround ? -1.0 : 4.0) * 0.3), 0.0, 1.0);
    if (!this.onGround && data.flapping < 1.0) {
      data.flapping = 1.0;
    }

    data.flapping *= 0.9;
    data.flap += data.flapping * 2.0;

    if (this.isAlive() && !this.isBaby() && !data.isChickenJockey) {
      data.eggLayTime--;
      if (data.eggLayTime <= 0) {
        // Entity runtime: egg item spawning and chicken egg sound wait for item entities and sound events.
        data.eggLayTime = 6000 + this.random.nextInt(6000);
      }
    }
  }

  private isBaby(): boolean {
    return this.age < 0;
  }

  private applyControlledTravel(): void {
    if (this.speed <= 0.0 || this.zza === 0.0) {
      return;
    }

    const targetX = this.moveControl.getWantedX();
    const targetY = this.moveControl.getWantedY();
    const targetZ = this.moveControl.getWantedZ();
    const dx = targetX - this.position.x;
    const dz = targetZ - this.position.z;
    const horizontalDistance = Math.sqrt((dx * dx) + (dz * dz));
    if (horizontalDistance <= 1.0e-6) {
      this.setZza(0.0);
      return;
    }

    const step = Math.min(Math.abs(this.speed * this.zza), horizontalDistance);
    const nextX = this.position.x + ((dx / horizontalDistance) * step);
    const nextZ = this.position.z + ((dz / horizontalDistance) * step);
    const stableY = this.aiLevel?.findStableStandingY(nextX, nextZ, targetY);
    if (stableY === undefined && this.aiLevel !== undefined) {
      this.navigation.stop();
      this.setZza(0.0);
      return;
    }

    const nextY = stableY ?? this.position.y;
    if (this.aiLevel !== undefined && !this.aiLevel.noCollision(this, this.createMobBoundingBox(nextX, nextY, nextZ))) {
      this.navigation.stop();
      this.setZza(0.0);
      return;
    }

    this.setPosition(nextX, nextY, nextZ);
    if (stableY !== undefined) {
      this.onGround = true;
    }
  }

  private createMobBoundingBox(x: number, y: number, z: number): AABB {
    const halfWidth = this.getBbWidth() / 2.0;
    return new AABB(
      x - halfWidth,
      y,
      z - halfWidth,
      x + halfWidth,
      y + this.getBbHeight(),
      z + halfWidth,
    );
  }
}

export interface EntityTypeOptions {
  readonly id: string;
  readonly category: MobCategory;
  readonly width: number;
  readonly height: number;
  readonly eyeHeight?: number;
  readonly attributes?: Partial<Record<MobAttributeValue, number>>;
  readonly canSummon?: boolean;
  readonly canSpawnFarFromPlayer?: boolean;
  readonly defaultData?: (random: { nextInt(bound: number): number }) => Readonly<Record<string, number | boolean | string>>;
}

export class EntityType {
  public readonly id: string;
  public readonly category: MobCategory;
  public readonly width: number;
  public readonly height: number;
  public readonly eyeHeight: number;
  private readonly attributes: Readonly<Record<MobAttributeValue, number>>;
  private readonly summon: boolean;
  private readonly spawnFarFromPlayer: boolean;
  private readonly defaultDataFactory: ((random: { nextInt(bound: number): number }) => Readonly<Record<string, number | boolean | string>>) | undefined;

  public constructor(options: EntityTypeOptions) {
    this.id = options.id;
    this.category = options.category;
    this.width = options.width;
    this.height = options.height;
    this.eyeHeight = options.eyeHeight ?? options.height * 0.85;
    this.attributes = {
      ...DEFAULT_MOB_ATTRIBUTES,
      ...options.attributes,
    };
    this.summon = options.canSummon ?? true;
    this.spawnFarFromPlayer = options.canSpawnFarFromPlayer ?? false;
    this.defaultDataFactory = options.defaultData;
  }

  public canSummon(): boolean {
    return this.summon;
  }

  public canSpawnFarFromPlayer(): boolean {
    return this.spawnFarFromPlayer;
  }

  public getAttributeValue(attribute: MobAttributeValue): number {
    return this.attributes[attribute];
  }

  public createDefaultData(random: { nextInt(bound: number): number }): Readonly<Record<string, number | boolean | string>> {
    return this.defaultDataFactory?.(random) ?? {};
  }

  public createGeneratedMob(
    id: number,
    uuid: string,
    x: number,
    y: number,
    z: number,
    yaw: number,
    pitch: number,
    random: { nextInt(bound: number): number },
  ): GeneratedMobEntity {
    return new GeneratedMobEntity({
      id,
      uuid,
      entityType: this,
      x,
      y,
      z,
      yaw,
      pitch,
      data: this.createDefaultData(random),
    });
  }

  public getAABB(x: number, y: number, z: number): AABB {
    const halfWidth = this.width / 2;
    return new AABB(x - halfWidth, y, z - halfWidth, x + halfWidth, y + this.height, z + halfWidth);
  }
}

function sheepColor(random: { nextInt(bound: number): number }): number {
  const colorRoll = random.nextInt(100);
  if (colorRoll < 5) {
    return 15;
  }
  if (colorRoll < 10) {
    return 7;
  }
  if (colorRoll < 15) {
    return 8;
  }
  if (colorRoll < 18) {
    return 12;
  }
  return random.nextInt(500) === 0 ? 6 : 0;
}

function creature(id: string, width: number, height: number, options: Omit<EntityTypeOptions, "id" | "category" | "width" | "height"> = {}): EntityType {
  return new EntityType({ id, category: MobCategory.CREATURE, width, height, ...options });
}

function chickenData(random: { nextInt(bound: number): number }): Readonly<Record<string, number | boolean | string>> {
  return {
    Flap: 0.0,
    FlapSpeed: 0.0,
    OFlap: 0.0,
    OFlapSpeed: 0.0,
    Flapping: 1.0,
    EggLayTime: 6000 + random.nextInt(6000),
    IsChickenJockey: false,
  };
}

function createChickenRuntimeData(
  data: Readonly<Record<string, number | boolean | string>>,
  random: { nextInt(bound: number): number },
): GeneratedChickenRuntimeData {
  return {
    flap: readNumberData(data.Flap, 0.0),
    flapSpeed: readNumberData(data.FlapSpeed, 0.0),
    oFlap: readNumberData(data.OFlap, 0.0),
    oFlapSpeed: readNumberData(data.OFlapSpeed, 0.0),
    flapping: readNumberData(data.Flapping, 1.0),
    eggLayTime: readIntegerData(data.EggLayTime, 6000 + random.nextInt(6000)),
    isChickenJockey: data.IsChickenJockey === true,
  };
}

function createChickenRuntimeDataSnapshot(data: GeneratedChickenRuntimeData): Readonly<Record<string, number | boolean | string>> {
  return {
    Flap: data.flap,
    FlapSpeed: data.flapSpeed,
    OFlap: data.oFlap,
    OFlapSpeed: data.oFlapSpeed,
    Flapping: data.flapping,
    EggLayTime: data.eggLayTime,
    IsChickenJockey: data.isChickenJockey,
  };
}

function readNumberData(value: number | boolean | string | undefined, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function readIntegerData(value: number | boolean | string | undefined, fallback: number): number {
  return Math.trunc(readNumberData(value, fallback));
}

export const EntityTypes = {
  SHEEP: creature("minecraft:sheep", 0.9, 1.3, {
    eyeHeight: 1.235,
    attributes: { [MobAttribute.MAX_HEALTH]: 8.0, [MobAttribute.MOVEMENT_SPEED]: 0.23 },
    defaultData: (random) => ({ Color: sheepColor(random) }),
  }),
  PIG: creature("minecraft:pig", 0.9, 0.9, { attributes: { [MobAttribute.MAX_HEALTH]: 10.0, [MobAttribute.MOVEMENT_SPEED]: 0.25 } }),
  CHICKEN: creature("minecraft:chicken", 0.4, 0.7, {
    eyeHeight: 0.644,
    attributes: { [MobAttribute.MAX_HEALTH]: 4.0, [MobAttribute.MOVEMENT_SPEED]: 0.25 },
    defaultData: chickenData,
  }),
  COW: creature("minecraft:cow", 0.9, 1.4, {
    eyeHeight: 1.3,
    attributes: { [MobAttribute.MAX_HEALTH]: 10.0, [MobAttribute.MOVEMENT_SPEED]: 0.2 },
  }),
  WOLF: creature("minecraft:wolf", 0.6, 0.85),
  RABBIT: creature("minecraft:rabbit", 0.4, 0.5),
  FOX: creature("minecraft:fox", 0.6, 0.7),
  HORSE: creature("minecraft:horse", 1.3964844, 1.6),
  DONKEY: creature("minecraft:donkey", 1.3964844, 1.5),
  LLAMA: creature("minecraft:llama", 0.9, 1.87),
  GOAT: creature("minecraft:goat", 0.9, 1.3),
  POLAR_BEAR: creature("minecraft:polar_bear", 1.4, 1.4),
  MOOSHROOM: creature("minecraft:mooshroom", 0.9, 1.4),
} as const;

const ENTITY_TYPES_BY_ID = new Map(Object.values(EntityTypes).map((type) => [type.id, type] as const));

export function getEntityType(id: string): EntityType | undefined {
  return ENTITY_TYPES_BY_ID.get(id);
}

function generatedEntityRandomSeed(id: number, uuid: string): bigint {
  let hash = BigInt(id);
  for (let index = 0; index < uuid.length; index++) {
    hash = BigInt.asIntN(64, (hash * 31n) + BigInt(uuid.charCodeAt(index)));
  }
  return hash;
}
