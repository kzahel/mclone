import { SyntheticRuntimeEntity, type SyntheticRuntimeEntityOptions } from "../level/entity/entity-access";
import { AABB } from "../phys/aabb";
import { BlockPos } from "../../core/block-pos";
import { DEFAULT_MOB_ATTRIBUTES, MobAttribute, type MobAttribute as MobAttributeValue } from "./attribute";
import { GoalSelector } from "./ai/goal/goal-selector";
import { WaterAvoidingRandomStrollGoal } from "./ai/goal/water-avoiding-random-stroll-goal";
import { MoveControl } from "./ai/control/move-control";
import { SimpleGroundPathNavigation } from "./ai/navigation/simple-ground-path-navigation";
import type { MobAiLevel, MobRandom, PathfinderMob } from "./ai/pathfinder-mob";
import { MobCategory } from "./mob-category";
import { WorldgenRandom } from "../../worldgen/prng/worldgen-random";

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
  public readonly navigation: SimpleGroundPathNavigation;
  public readonly moveControl: MoveControl;
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

  public constructor(options: GeneratedMobEntityOptions) {
    super({
      ...options,
      typeId: options.entityType.id,
      width: options.entityType.width,
      height: options.entityType.height,
    });
    this.entityType = options.entityType;
    this.navigation = new SimpleGroundPathNavigation(this);
    this.moveControl = new MoveControl(this);
    this.age = options.age ?? 0;
    this.onGround = options.onGround ?? false;
    this.data = options.data ?? {};
    this.random = new WorldgenRandom(options.randomSeed ?? generatedEntityRandomSeed(options.id, options.uuid));
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
    if (options.resetNoActionTime) {
      this.noActionTime = 0;
    }

    this.noActionTime++;
    this.goalSelector.tick();
    this.navigation.tick();
    this.moveControl.tick();
    this.applyControlledTravel();
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

  public getYRot(): number {
    return this.rotation.yaw;
  }

  public setYRot(yaw: number): void {
    this.setRotation(yaw, this.rotation.pitch);
  }

  public getBbWidth(): number {
    return this.entityType.width;
  }

  public getRandom(): MobRandom {
    return this.random;
  }

  public getNoActionTime(): number {
    return this.noActionTime;
  }

  public getNavigation(): SimpleGroundPathNavigation {
    return this.navigation;
  }

  public getMoveControl(): MoveControl {
    return this.moveControl;
  }

  public getAttributeValue(attribute: MobAttributeValue): number {
    return this.entityType.getAttributeValue(attribute);
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
    // Runtime: BlockPathTypes/WalkNodeEvaluator malus table is deferred to the full pathfinding port.
    return false;
  }

  public resetNoActionTime(): void {
    this.noActionTime = 0;
  }

  private registerGoals(): void {
    if (this.entityType.id === EntityTypes.COW.id) {
      this.goalSelector.addGoal(5, new WaterAvoidingRandomStrollGoal(this, 1.0));
    }
  }

  private applyControlledTravel(): void {
    const target = this.navigation.getTarget();
    if (target === undefined || this.speed <= 0.0 || this.zza === 0.0) {
      return;
    }

    const dx = target.x - this.position.x;
    const dz = target.z - this.position.z;
    const horizontalDistance = Math.sqrt((dx * dx) + (dz * dz));
    if (horizontalDistance <= 1.0e-6) {
      this.navigation.stop();
      return;
    }

    const step = Math.min(Math.abs(this.speed * this.zza), horizontalDistance);
    const yStep = Math.abs(target.y - this.position.y) <= step
      ? target.y - this.position.y
      : Math.sign(target.y - this.position.y) * step;
    this.setPosition(
      this.position.x + ((dx / horizontalDistance) * step),
      this.position.y + yStep,
      this.position.z + ((dz / horizontalDistance) * step),
    );

    if (step >= horizontalDistance) {
      this.navigation.stop();
    }
  }
}

export interface EntityTypeOptions {
  readonly id: string;
  readonly category: MobCategory;
  readonly width: number;
  readonly height: number;
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
  private readonly attributes: Readonly<Record<MobAttributeValue, number>>;
  private readonly summon: boolean;
  private readonly spawnFarFromPlayer: boolean;
  private readonly defaultDataFactory: ((random: { nextInt(bound: number): number }) => Readonly<Record<string, number | boolean | string>>) | undefined;

  public constructor(options: EntityTypeOptions) {
    this.id = options.id;
    this.category = options.category;
    this.width = options.width;
    this.height = options.height;
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
      data: this.defaultDataFactory?.(random),
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

export const EntityTypes = {
  SHEEP: creature("minecraft:sheep", 0.9, 1.3, { defaultData: (random) => ({ Color: sheepColor(random) }) }),
  PIG: creature("minecraft:pig", 0.9, 0.9),
  CHICKEN: creature("minecraft:chicken", 0.4, 0.7),
  COW: creature("minecraft:cow", 0.9, 1.4, { attributes: { [MobAttribute.MAX_HEALTH]: 10.0, [MobAttribute.MOVEMENT_SPEED]: 0.2 } }),
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
