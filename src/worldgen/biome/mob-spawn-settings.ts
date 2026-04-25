import { EntityTypes, type EntityType } from "../../world/entity/entity-type";
import { MOB_CATEGORIES, MobCategory, type MobCategory as MobCategoryValue } from "../../world/entity/mob-category";

export class WeightedRandomList<T extends { readonly weight: number }> {
  private readonly totalWeight: number;

  private constructor(private readonly items: readonly T[]) {
    this.totalWeight = items.reduce((sum, item) => sum + item.weight, 0);
  }

  public static create<T extends { readonly weight: number }>(items: readonly T[] = []): WeightedRandomList<T> {
    return new WeightedRandomList(items);
  }

  public isEmpty(): boolean {
    return this.items.length === 0;
  }

  public getRandom(random: { nextInt(bound: number): number }): T | undefined {
    if (this.totalWeight === 0) {
      return undefined;
    }

    let target = random.nextInt(this.totalWeight);
    for (const item of this.items) {
      target -= item.weight;
      if (target < 0) {
        return item;
      }
    }

    return undefined;
  }

  public unwrap(): readonly T[] {
    return this.items;
  }
}

export class SpawnerData {
  public readonly type: EntityType;
  public readonly weight: number;
  public readonly minCount: number;
  public readonly maxCount: number;

  public constructor(type: EntityType, weight: number, minCount: number, maxCount: number) {
    this.type = type.category === MobCategory.MISC ? EntityTypes.PIG : type;
    this.weight = weight;
    this.minCount = minCount;
    this.maxCount = maxCount;
  }
}

export class MobSpawnSettings {
  public static readonly EMPTY = new MobSpawnSettings();

  public constructor(
    private readonly creatureGenerationProbability = 0.1,
    private readonly spawners: ReadonlyMap<MobCategoryValue, WeightedRandomList<SpawnerData>> = emptySpawnerMap(),
  ) {}

  public getMobs(category: MobCategoryValue): WeightedRandomList<SpawnerData> {
    return this.spawners.get(category) ?? WeightedRandomList.create();
  }

  public getCreatureProbability(): number {
    return this.creatureGenerationProbability;
  }
}

export class MobSpawnSettingsBuilder {
  private readonly spawners = new Map<MobCategoryValue, SpawnerData[]>(
    MOB_CATEGORIES.map((category) => [category, []] as const),
  );
  private creatureGenerationProbabilityValue = 0.1;

  public addSpawn(category: MobCategoryValue, data: SpawnerData): this {
    this.spawners.get(category)?.push(data);
    return this;
  }

  public creatureGenerationProbability(probability: number): this {
    this.creatureGenerationProbabilityValue = probability;
    return this;
  }

  public build(): MobSpawnSettings {
    return new MobSpawnSettings(
      this.creatureGenerationProbabilityValue,
      new Map([...this.spawners].map(([category, data]) => [category, WeightedRandomList.create(data)] as const)),
    );
  }
}

function emptySpawnerMap(): ReadonlyMap<MobCategoryValue, WeightedRandomList<SpawnerData>> {
  return new Map(MOB_CATEGORIES.map((category) => [category, WeightedRandomList.create<SpawnerData>()] as const));
}
