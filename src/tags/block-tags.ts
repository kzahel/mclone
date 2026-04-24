import { ResourceLocation } from "../core/resource-location";
import type { Block } from "../world/level/block/block";

function normalizeEntries(entries: readonly (ResourceLocation | string)[]): ReadonlySet<string> {
  return new Set(entries.map((entry) => (entry instanceof ResourceLocation ? entry.toString() : entry)));
}

export class BlockTag {
  private readonly entries: ReadonlySet<string>;

  public constructor(
    private readonly name: string,
    entries: readonly (ResourceLocation | string)[],
  ) {
    this.entries = normalizeEntries(entries);
  }

  public contains(block: Block): boolean {
    const location = block.getLocation();
    return location !== undefined && this.entries.has(location.toString());
  }

  public toString(): string {
    return this.name;
  }
}

export class BlockTags {
  public static readonly BAMBOO_PLANTABLE_ON = new BlockTag("minecraft:bamboo_plantable_on", [
    "minecraft:sand",
    "minecraft:red_sand",
    "minecraft:dirt",
    "minecraft:grass_block",
    "minecraft:podzol",
    "minecraft:coarse_dirt",
    "minecraft:mycelium",
    "minecraft:bamboo",
    "minecraft:bamboo_sapling",
    "minecraft:gravel",
  ]);

  public static readonly CORAL_BLOCKS = new BlockTag("minecraft:coral_blocks", [
    "minecraft:tube_coral_block",
    "minecraft:brain_coral_block",
    "minecraft:bubble_coral_block",
    "minecraft:fire_coral_block",
    "minecraft:horn_coral_block",
  ]);

  public static readonly CORALS = new BlockTag("minecraft:corals", [
    "minecraft:tube_coral",
    "minecraft:brain_coral",
    "minecraft:bubble_coral",
    "minecraft:fire_coral",
    "minecraft:horn_coral",
    "minecraft:tube_coral_fan",
    "minecraft:brain_coral_fan",
    "minecraft:bubble_coral_fan",
    "minecraft:fire_coral_fan",
    "minecraft:horn_coral_fan",
  ]);

  public static readonly WALL_CORALS = new BlockTag("minecraft:wall_corals", [
    "minecraft:tube_coral_wall_fan",
    "minecraft:brain_coral_wall_fan",
    "minecraft:bubble_coral_wall_fan",
    "minecraft:fire_coral_wall_fan",
    "minecraft:horn_coral_wall_fan",
  ]);

  public static readonly DIRT = new BlockTag("minecraft:dirt", [
    "minecraft:dirt",
    "minecraft:grass_block",
    "minecraft:podzol",
    "minecraft:coarse_dirt",
    "minecraft:mycelium",
    "minecraft:rooted_dirt",
    "minecraft:moss_block",
  ]);

  public static readonly MUSHROOM_GROW_BLOCK = new BlockTag("minecraft:mushroom_grow_block", [
    "minecraft:mycelium",
    "minecraft:podzol",
    "minecraft:crimson_nylium",
    "minecraft:warped_nylium",
  ]);

  public static readonly LOGS = new BlockTag("minecraft:logs", [
    "minecraft:oak_log",
    "minecraft:spruce_log",
    "minecraft:birch_log",
    "minecraft:jungle_log",
    "minecraft:acacia_log",
    "minecraft:dark_oak_log",
    "minecraft:oak_wood",
    "minecraft:spruce_wood",
    "minecraft:birch_wood",
    "minecraft:jungle_wood",
    "minecraft:acacia_wood",
    "minecraft:dark_oak_wood",
    "minecraft:stripped_oak_log",
    "minecraft:stripped_spruce_log",
    "minecraft:stripped_birch_log",
    "minecraft:stripped_jungle_log",
    "minecraft:stripped_acacia_log",
    "minecraft:stripped_dark_oak_log",
    "minecraft:stripped_oak_wood",
    "minecraft:stripped_spruce_wood",
    "minecraft:stripped_birch_wood",
    "minecraft:stripped_jungle_wood",
    "minecraft:stripped_acacia_wood",
    "minecraft:stripped_dark_oak_wood",
  ]);

  public static readonly JUNGLE_LOGS = new BlockTag("minecraft:jungle_logs", [
    "minecraft:jungle_log",
    "minecraft:jungle_wood",
    "minecraft:stripped_jungle_log",
    "minecraft:stripped_jungle_wood",
  ]);

  public static readonly LEAVES = new BlockTag("minecraft:leaves", [
    "minecraft:oak_leaves",
    "minecraft:spruce_leaves",
    "minecraft:birch_leaves",
    "minecraft:jungle_leaves",
    "minecraft:acacia_leaves",
    "minecraft:dark_oak_leaves",
    "minecraft:azalea_leaves",
    "minecraft:flowering_azalea_leaves",
  ]);

  public static readonly BASE_STONE_OVERWORLD = new BlockTag("minecraft:base_stone_overworld", [
    "minecraft:stone",
    "minecraft:granite",
    "minecraft:diorite",
    "minecraft:andesite",
    "minecraft:tuff",
    "minecraft:deepslate",
  ]);

  public static readonly STONE_ORE_REPLACEABLES = new BlockTag("minecraft:stone_ore_replaceables", [
    "minecraft:stone",
    "minecraft:granite",
    "minecraft:diorite",
    "minecraft:andesite",
  ]);

  public static readonly DEEPSLATE_ORE_REPLACEABLES = new BlockTag("minecraft:deepslate_ore_replaceables", [
    "minecraft:deepslate",
    "minecraft:tuff",
  ]);

  public static readonly LAVA_POOL_STONE_CANNOT_REPLACE = new BlockTag("minecraft:lava_pool_stone_cannot_replace", []);
}
