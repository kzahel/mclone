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
pub const RED_SAND: RawBlockId = 38;
pub const ICE: RawBlockId = 39;
pub const SNOW_BLOCK: RawBlockId = 40;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct GeneratedBlockId(pub RawBlockId);

impl GeneratedBlockId {
    pub const AIR: Self = Self(AIR);
    pub const STONE: Self = Self(STONE);
    pub const WATER: Self = Self(WATER);
    pub const BEDROCK: Self = Self(BEDROCK);

    pub const fn raw(self) -> RawBlockId {
        self.0
    }

    pub const fn block_state_id(self) -> BlockStateId {
        generated_block_state_id(self.0)
    }

    pub const fn is_air(self) -> bool {
        self.0 == AIR
    }

    pub const fn is_water(self) -> bool {
        self.0 == WATER
    }

    pub const fn name(self) -> &'static str {
        block_name(self.0)
    }
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
        RED_SAND => "minecraft:red_sand",
        ICE => "minecraft:ice",
        SNOW_BLOCK => "minecraft:snow_block",
        _ => "minecraft:unknown",
    }
}
