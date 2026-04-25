import { SyntheticRuntimeEntity, type SyntheticRuntimeEntityOptions } from "../level/entity/entity-access";
import { AABB } from "../phys/aabb";
import { MobCategory } from "./mob-category";

export interface GeneratedMobEntityOptions extends SyntheticRuntimeEntityOptions {
  readonly entityType: EntityType;
  readonly age?: number;
  readonly onGround?: boolean;
  readonly data?: Readonly<Record<string, number | boolean | string>>;
}

export class GeneratedMobEntity extends SyntheticRuntimeEntity {
  public readonly entityType: EntityType;
  public readonly age: number;
  public readonly onGround: boolean;
  public readonly data: Readonly<Record<string, number | boolean | string>>;

  public constructor(options: GeneratedMobEntityOptions) {
    super({
      ...options,
      typeId: options.entityType.id,
      width: options.entityType.width,
      height: options.entityType.height,
    });
    this.entityType = options.entityType;
    this.age = options.age ?? 0;
    this.onGround = options.onGround ?? false;
    this.data = options.data ?? {};
  }
}

export interface EntityTypeOptions {
  readonly id: string;
  readonly category: MobCategory;
  readonly width: number;
  readonly height: number;
  readonly canSummon?: boolean;
  readonly canSpawnFarFromPlayer?: boolean;
  readonly defaultData?: (random: { nextInt(bound: number): number }) => Readonly<Record<string, number | boolean | string>>;
}

export class EntityType {
  public readonly id: string;
  public readonly category: MobCategory;
  public readonly width: number;
  public readonly height: number;
  private readonly summon: boolean;
  private readonly spawnFarFromPlayer: boolean;
  private readonly defaultDataFactory: ((random: { nextInt(bound: number): number }) => Readonly<Record<string, number | boolean | string>>) | undefined;

  public constructor(options: EntityTypeOptions) {
    this.id = options.id;
    this.category = options.category;
    this.width = options.width;
    this.height = options.height;
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
  COW: creature("minecraft:cow", 0.9, 1.4),
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
