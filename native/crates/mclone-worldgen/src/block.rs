use mclone_core::BlockStateId;

pub type RawBlockId = u8;

pub const AIR: RawBlockId = 0;
pub const STONE: RawBlockId = 1;
pub const WATER: RawBlockId = 2;
pub const BEDROCK: RawBlockId = 3;
pub const GRASS_BLOCK: RawBlockId = 4;
pub const DIRT: RawBlockId = 5;
pub const SAND: RawBlockId = 6;
pub const GRAVEL: RawBlockId = 7;
pub const SNOW: RawBlockId = 8;
pub const LAVA: RawBlockId = 9;
pub const GRANITE: RawBlockId = 10;
pub const DIORITE: RawBlockId = 11;
pub const ANDESITE: RawBlockId = 12;
pub const COARSE_DIRT: RawBlockId = 13;
pub const PODZOL: RawBlockId = 14;
pub const MYCELIUM: RawBlockId = 15;
pub const TERRACOTTA: RawBlockId = 16;
pub const WHITE_TERRACOTTA: RawBlockId = 17;
pub const ORANGE_TERRACOTTA: RawBlockId = 18;
pub const MAGENTA_TERRACOTTA: RawBlockId = 19;
pub const LIGHT_BLUE_TERRACOTTA: RawBlockId = 20;
pub const YELLOW_TERRACOTTA: RawBlockId = 21;
pub const LIME_TERRACOTTA: RawBlockId = 22;
pub const PINK_TERRACOTTA: RawBlockId = 23;
pub const GRAY_TERRACOTTA: RawBlockId = 24;
pub const LIGHT_GRAY_TERRACOTTA: RawBlockId = 25;
pub const CYAN_TERRACOTTA: RawBlockId = 26;
pub const PURPLE_TERRACOTTA: RawBlockId = 27;
pub const BLUE_TERRACOTTA: RawBlockId = 28;
pub const BROWN_TERRACOTTA: RawBlockId = 29;
pub const GREEN_TERRACOTTA: RawBlockId = 30;
pub const RED_TERRACOTTA: RawBlockId = 31;
pub const BLACK_TERRACOTTA: RawBlockId = 32;
pub const SANDSTONE: RawBlockId = 33;
pub const RED_SANDSTONE: RawBlockId = 34;
pub const PACKED_ICE: RawBlockId = 35;
pub const OBSIDIAN: RawBlockId = 36;
pub const MAGMA_BLOCK: RawBlockId = 37;
pub const RED_SAND: RawBlockId = 38;
pub const ICE: RawBlockId = 39;
pub const SNOW_BLOCK: RawBlockId = 40;
pub const OAK_LOG: RawBlockId = 41;
pub const OAK_LEAVES: RawBlockId = 42;
pub const GRASS: RawBlockId = 43;
pub const DANDELION: RawBlockId = 44;
pub const POPPY: RawBlockId = 45;
pub const BIRCH_LOG: RawBlockId = 46;
pub const BIRCH_LEAVES: RawBlockId = 47;
pub const SPRUCE_LOG: RawBlockId = 48;
pub const SPRUCE_LEAVES: RawBlockId = 49;
pub const FERN: RawBlockId = 50;
pub const DEAD_BUSH: RawBlockId = 51;
pub const TUFF: RawBlockId = 52;
pub const DEEPSLATE: RawBlockId = 53;
pub const COAL_ORE: RawBlockId = 54;
pub const DEEPSLATE_COAL_ORE: RawBlockId = 55;
pub const COPPER_ORE: RawBlockId = 56;
pub const DEEPSLATE_COPPER_ORE: RawBlockId = 57;
pub const IRON_ORE: RawBlockId = 58;
pub const DEEPSLATE_IRON_ORE: RawBlockId = 59;
pub const GOLD_ORE: RawBlockId = 60;
pub const DEEPSLATE_GOLD_ORE: RawBlockId = 61;
pub const REDSTONE_ORE: RawBlockId = 62;
pub const DEEPSLATE_REDSTONE_ORE: RawBlockId = 63;
pub const DIAMOND_ORE: RawBlockId = 64;
pub const DEEPSLATE_DIAMOND_ORE: RawBlockId = 65;
pub const LAPIS_ORE: RawBlockId = 66;
pub const DEEPSLATE_LAPIS_ORE: RawBlockId = 67;
pub const LARGE_FERN_LOWER: RawBlockId = 68;
pub const LARGE_FERN_UPPER: RawBlockId = 69;
pub const GLOW_LICHEN: RawBlockId = 70;
pub const CAVE_AIR: RawBlockId = 71;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct GeneratedBlockId(pub RawBlockId);

impl GeneratedBlockId {
    pub const AIR: Self = Self(AIR);
    pub const STONE: Self = Self(STONE);
    pub const WATER: Self = Self(WATER);
    pub const BEDROCK: Self = Self(BEDROCK);
    pub const TUFF: Self = Self(TUFF);
    pub const DEEPSLATE: Self = Self(DEEPSLATE);
    pub const COAL_ORE: Self = Self(COAL_ORE);
    pub const DEEPSLATE_COAL_ORE: Self = Self(DEEPSLATE_COAL_ORE);
    pub const COPPER_ORE: Self = Self(COPPER_ORE);
    pub const DEEPSLATE_COPPER_ORE: Self = Self(DEEPSLATE_COPPER_ORE);
    pub const IRON_ORE: Self = Self(IRON_ORE);
    pub const DEEPSLATE_IRON_ORE: Self = Self(DEEPSLATE_IRON_ORE);
    pub const GOLD_ORE: Self = Self(GOLD_ORE);
    pub const DEEPSLATE_GOLD_ORE: Self = Self(DEEPSLATE_GOLD_ORE);
    pub const REDSTONE_ORE: Self = Self(REDSTONE_ORE);
    pub const DEEPSLATE_REDSTONE_ORE: Self = Self(DEEPSLATE_REDSTONE_ORE);
    pub const DIAMOND_ORE: Self = Self(DIAMOND_ORE);
    pub const DEEPSLATE_DIAMOND_ORE: Self = Self(DEEPSLATE_DIAMOND_ORE);
    pub const LAPIS_ORE: Self = Self(LAPIS_ORE);
    pub const DEEPSLATE_LAPIS_ORE: Self = Self(DEEPSLATE_LAPIS_ORE);
    pub const LARGE_FERN_LOWER: Self = Self(LARGE_FERN_LOWER);
    pub const LARGE_FERN_UPPER: Self = Self(LARGE_FERN_UPPER);
    pub const GLOW_LICHEN: Self = Self(GLOW_LICHEN);
    pub const CAVE_AIR: Self = Self(CAVE_AIR);

    pub const fn raw(self) -> RawBlockId {
        self.0
    }

    pub const fn block_state_id(self) -> BlockStateId {
        generated_block_state_id(self.0)
    }

    pub const fn is_air(self) -> bool {
        is_air_like(self.0)
    }

    pub const fn is_water(self) -> bool {
        self.0 == WATER
    }

    pub const fn name(self) -> &'static str {
        block_name(self.0)
    }
}

pub const fn is_air_like(block_id: RawBlockId) -> bool {
    block_id == AIR || block_id == CAVE_AIR
}

pub const fn generated_block_state_id(block_id: RawBlockId) -> BlockStateId {
    BlockStateId(block_id as u32)
}

pub const fn block_name(block_id: RawBlockId) -> &'static str {
    match block_id {
        AIR => "minecraft:air",
        STONE => "minecraft:stone",
        WATER => "minecraft:water",
        BEDROCK => "minecraft:bedrock",
        GRASS_BLOCK => "minecraft:grass_block",
        DIRT => "minecraft:dirt",
        SAND => "minecraft:sand",
        GRAVEL => "minecraft:gravel",
        SNOW => "minecraft:snow",
        LAVA => "minecraft:lava",
        GRANITE => "minecraft:granite",
        DIORITE => "minecraft:diorite",
        ANDESITE => "minecraft:andesite",
        COARSE_DIRT => "minecraft:coarse_dirt",
        PODZOL => "minecraft:podzol",
        MYCELIUM => "minecraft:mycelium",
        TERRACOTTA => "minecraft:terracotta",
        WHITE_TERRACOTTA => "minecraft:white_terracotta",
        ORANGE_TERRACOTTA => "minecraft:orange_terracotta",
        MAGENTA_TERRACOTTA => "minecraft:magenta_terracotta",
        LIGHT_BLUE_TERRACOTTA => "minecraft:light_blue_terracotta",
        YELLOW_TERRACOTTA => "minecraft:yellow_terracotta",
        LIME_TERRACOTTA => "minecraft:lime_terracotta",
        PINK_TERRACOTTA => "minecraft:pink_terracotta",
        GRAY_TERRACOTTA => "minecraft:gray_terracotta",
        LIGHT_GRAY_TERRACOTTA => "minecraft:light_gray_terracotta",
        CYAN_TERRACOTTA => "minecraft:cyan_terracotta",
        PURPLE_TERRACOTTA => "minecraft:purple_terracotta",
        BLUE_TERRACOTTA => "minecraft:blue_terracotta",
        BROWN_TERRACOTTA => "minecraft:brown_terracotta",
        GREEN_TERRACOTTA => "minecraft:green_terracotta",
        RED_TERRACOTTA => "minecraft:red_terracotta",
        BLACK_TERRACOTTA => "minecraft:black_terracotta",
        SANDSTONE => "minecraft:sandstone",
        RED_SANDSTONE => "minecraft:red_sandstone",
        PACKED_ICE => "minecraft:packed_ice",
        OBSIDIAN => "minecraft:obsidian",
        MAGMA_BLOCK => "minecraft:magma_block",
        RED_SAND => "minecraft:red_sand",
        ICE => "minecraft:ice",
        SNOW_BLOCK => "minecraft:snow_block",
        OAK_LOG => "minecraft:oak_log",
        OAK_LEAVES => "minecraft:oak_leaves",
        GRASS => "minecraft:grass",
        DANDELION => "minecraft:dandelion",
        POPPY => "minecraft:poppy",
        BIRCH_LOG => "minecraft:birch_log",
        BIRCH_LEAVES => "minecraft:birch_leaves",
        SPRUCE_LOG => "minecraft:spruce_log",
        SPRUCE_LEAVES => "minecraft:spruce_leaves",
        FERN => "minecraft:fern",
        DEAD_BUSH => "minecraft:dead_bush",
        TUFF => "minecraft:tuff",
        DEEPSLATE => "minecraft:deepslate",
        COAL_ORE => "minecraft:coal_ore",
        DEEPSLATE_COAL_ORE => "minecraft:deepslate_coal_ore",
        COPPER_ORE => "minecraft:copper_ore",
        DEEPSLATE_COPPER_ORE => "minecraft:deepslate_copper_ore",
        IRON_ORE => "minecraft:iron_ore",
        DEEPSLATE_IRON_ORE => "minecraft:deepslate_iron_ore",
        GOLD_ORE => "minecraft:gold_ore",
        DEEPSLATE_GOLD_ORE => "minecraft:deepslate_gold_ore",
        REDSTONE_ORE => "minecraft:redstone_ore",
        DEEPSLATE_REDSTONE_ORE => "minecraft:deepslate_redstone_ore",
        DIAMOND_ORE => "minecraft:diamond_ore",
        DEEPSLATE_DIAMOND_ORE => "minecraft:deepslate_diamond_ore",
        LAPIS_ORE => "minecraft:lapis_ore",
        DEEPSLATE_LAPIS_ORE => "minecraft:deepslate_lapis_ore",
        LARGE_FERN_LOWER | LARGE_FERN_UPPER => "minecraft:large_fern",
        GLOW_LICHEN => "minecraft:glow_lichen",
        CAVE_AIR => "minecraft:cave_air",
        _ => "minecraft:unknown",
    }
}
