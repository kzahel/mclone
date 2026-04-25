export const MobCategory = {
  MONSTER: "monster",
  CREATURE: "creature",
  AMBIENT: "ambient",
  UNDERGROUND_WATER_CREATURE: "underground_water_creature",
  WATER_CREATURE: "water_creature",
  WATER_AMBIENT: "water_ambient",
  MISC: "misc",
} as const;

export type MobCategory = typeof MobCategory[keyof typeof MobCategory];

export interface MobCategoryProperties {
  readonly max: number;
  readonly friendly: boolean;
  readonly persistent: boolean;
  readonly despawnDistance: number;
  readonly noDespawnDistance: number;
}

const MOB_CATEGORY_PROPERTIES: Readonly<Record<MobCategory, MobCategoryProperties>> = {
  [MobCategory.MONSTER]: { max: 70, friendly: false, persistent: false, despawnDistance: 128, noDespawnDistance: 32 },
  [MobCategory.CREATURE]: { max: 10, friendly: true, persistent: true, despawnDistance: 128, noDespawnDistance: 32 },
  [MobCategory.AMBIENT]: { max: 15, friendly: true, persistent: false, despawnDistance: 128, noDespawnDistance: 32 },
  [MobCategory.UNDERGROUND_WATER_CREATURE]: { max: 5, friendly: true, persistent: false, despawnDistance: 128, noDespawnDistance: 32 },
  [MobCategory.WATER_CREATURE]: { max: 5, friendly: true, persistent: false, despawnDistance: 128, noDespawnDistance: 32 },
  [MobCategory.WATER_AMBIENT]: { max: 20, friendly: true, persistent: false, despawnDistance: 64, noDespawnDistance: 32 },
  [MobCategory.MISC]: { max: -1, friendly: true, persistent: true, despawnDistance: 128, noDespawnDistance: 32 },
};

export const MOB_CATEGORIES = [
  MobCategory.MONSTER,
  MobCategory.CREATURE,
  MobCategory.AMBIENT,
  MobCategory.UNDERGROUND_WATER_CREATURE,
  MobCategory.WATER_CREATURE,
  MobCategory.WATER_AMBIENT,
  MobCategory.MISC,
] as const satisfies readonly MobCategory[];

export function mobCategoryProperties(category: MobCategory): MobCategoryProperties {
  return MOB_CATEGORY_PROPERTIES[category];
}
