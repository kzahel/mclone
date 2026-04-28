export const MobAttribute = {
  MAX_HEALTH: "max_health",
  MOVEMENT_SPEED: "movement_speed",
} as const;

export type MobAttribute = typeof MobAttribute[keyof typeof MobAttribute];

export const DEFAULT_MOB_ATTRIBUTES: Readonly<Record<MobAttribute, number>> = {
  [MobAttribute.MAX_HEALTH]: 20.0,
  [MobAttribute.MOVEMENT_SPEED]: 0.25,
};
