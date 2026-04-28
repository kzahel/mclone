class BlockPathTypeValue {
  public constructor(
    private readonly name: string,
    private readonly malus: number,
    private readonly ordinalValue: number,
  ) {}

  public getMalus(): number {
    return this.malus;
  }

  public ordinal(): number {
    return this.ordinalValue;
  }

  public toString(): string {
    return this.name;
  }
}

const VALUES = [
  new BlockPathTypeValue("BLOCKED", -1.0, 0),
  new BlockPathTypeValue("OPEN", 0.0, 1),
  new BlockPathTypeValue("WALKABLE", 0.0, 2),
  new BlockPathTypeValue("WALKABLE_DOOR", 0.0, 3),
  new BlockPathTypeValue("TRAPDOOR", 0.0, 4),
  new BlockPathTypeValue("POWDER_SNOW", 0.0, 5),
  new BlockPathTypeValue("FENCE", -1.0, 6),
  new BlockPathTypeValue("LAVA", -1.0, 7),
  new BlockPathTypeValue("WATER", 8.0, 8),
  new BlockPathTypeValue("WATER_BORDER", 8.0, 9),
  new BlockPathTypeValue("RAIL", 0.0, 10),
  new BlockPathTypeValue("UNPASSABLE_RAIL", -1.0, 11),
  new BlockPathTypeValue("DANGER_FIRE", 8.0, 12),
  new BlockPathTypeValue("DAMAGE_FIRE", 16.0, 13),
  new BlockPathTypeValue("DANGER_CACTUS", 8.0, 14),
  new BlockPathTypeValue("DAMAGE_CACTUS", -1.0, 15),
  new BlockPathTypeValue("DANGER_OTHER", 8.0, 16),
  new BlockPathTypeValue("DAMAGE_OTHER", -1.0, 17),
  new BlockPathTypeValue("DOOR_OPEN", 0.0, 18),
  new BlockPathTypeValue("DOOR_WOOD_CLOSED", -1.0, 19),
  new BlockPathTypeValue("DOOR_IRON_CLOSED", -1.0, 20),
  new BlockPathTypeValue("BREACH", 4.0, 21),
  new BlockPathTypeValue("LEAVES", -1.0, 22),
  new BlockPathTypeValue("STICKY_HONEY", 8.0, 23),
  new BlockPathTypeValue("COCOA", 0.0, 24),
] as const;

export const BlockPathTypes = {
  BLOCKED: VALUES[0],
  OPEN: VALUES[1],
  WALKABLE: VALUES[2],
  WALKABLE_DOOR: VALUES[3],
  TRAPDOOR: VALUES[4],
  POWDER_SNOW: VALUES[5],
  FENCE: VALUES[6],
  LAVA: VALUES[7],
  WATER: VALUES[8],
  WATER_BORDER: VALUES[9],
  RAIL: VALUES[10],
  UNPASSABLE_RAIL: VALUES[11],
  DANGER_FIRE: VALUES[12],
  DAMAGE_FIRE: VALUES[13],
  DANGER_CACTUS: VALUES[14],
  DAMAGE_CACTUS: VALUES[15],
  DANGER_OTHER: VALUES[16],
  DAMAGE_OTHER: VALUES[17],
  DOOR_OPEN: VALUES[18],
  DOOR_WOOD_CLOSED: VALUES[19],
  DOOR_IRON_CLOSED: VALUES[20],
  BREACH: VALUES[21],
  LEAVES: VALUES[22],
  STICKY_HONEY: VALUES[23],
  COCOA: VALUES[24],
  values(): readonly BlockPathTypeValue[] {
    return VALUES;
  },
} as const;

export type BlockPathTypes = (typeof VALUES)[number];
