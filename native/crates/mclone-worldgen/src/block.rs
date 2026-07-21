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
pub const WATER_LEVEL_1: RawBlockId = 72;
pub const WATER_LEVEL_2: RawBlockId = 73;
pub const WATER_LEVEL_3: RawBlockId = 74;
pub const WATER_LEVEL_4: RawBlockId = 75;
pub const WATER_LEVEL_5: RawBlockId = 76;
pub const WATER_LEVEL_6: RawBlockId = 77;
pub const WATER_LEVEL_7: RawBlockId = 78;
pub const WATER_LEVEL_8: RawBlockId = 79;
pub const LAVA_LEVEL_1: RawBlockId = 80;
pub const LAVA_LEVEL_2: RawBlockId = 81;
pub const LAVA_LEVEL_3: RawBlockId = 82;
pub const LAVA_LEVEL_4: RawBlockId = 83;
pub const LAVA_LEVEL_5: RawBlockId = 84;
pub const LAVA_LEVEL_6: RawBlockId = 85;
pub const LAVA_LEVEL_7: RawBlockId = 86;
pub const LAVA_LEVEL_8: RawBlockId = 87;
pub const CLAY: RawBlockId = 88;
pub const DRIPSTONE_BLOCK: RawBlockId = 89;
pub const POINTED_DRIPSTONE: RawBlockId = 90;
pub const BRICKS: RawBlockId = 91;
pub const OAK_LOG_X: RawBlockId = 92;
pub const OAK_LOG_Z: RawBlockId = 93;
pub const BIRCH_LOG_X: RawBlockId = 94;
pub const BIRCH_LOG_Z: RawBlockId = 95;
pub const SPRUCE_LOG_X: RawBlockId = 96;
pub const SPRUCE_LOG_Z: RawBlockId = 97;
pub const DEEPSLATE_X: RawBlockId = 98;
pub const DEEPSLATE_Z: RawBlockId = 99;
pub const TORCH: RawBlockId = 100;
pub const WALL_TORCH_NORTH: RawBlockId = 101;
pub const WALL_TORCH_EAST: RawBlockId = 102;
pub const WALL_TORCH_SOUTH: RawBlockId = 103;
pub const WALL_TORCH_WEST: RawBlockId = 104;
pub const CACTUS: RawBlockId = 105;
pub const SUGAR_CANE: RawBlockId = 106;
pub const SEAGRASS: RawBlockId = 107;
pub const TALL_SEAGRASS_LOWER: RawBlockId = 108;
pub const TALL_SEAGRASS_UPPER: RawBlockId = 109;
pub const KELP: RawBlockId = 110;
pub const KELP_PLANT: RawBlockId = 111;
pub const TUBE_CORAL_BLOCK: RawBlockId = 112;
pub const BRAIN_CORAL_BLOCK: RawBlockId = 113;
pub const BUBBLE_CORAL_BLOCK: RawBlockId = 114;
pub const FIRE_CORAL_BLOCK: RawBlockId = 115;
pub const HORN_CORAL_BLOCK: RawBlockId = 116;
pub const SEA_PICKLE_1: RawBlockId = 117;
pub const SEA_PICKLE_2: RawBlockId = 118;
pub const SEA_PICKLE_3: RawBlockId = 119;
pub const SEA_PICKLE_4: RawBlockId = 120;
pub const DARK_OAK_LOG: RawBlockId = 121;
pub const DARK_OAK_LEAVES: RawBlockId = 122;
pub const BROWN_MUSHROOM_BLOCK: RawBlockId = 123;
pub const RED_MUSHROOM_BLOCK: RawBlockId = 124;
pub const MUSHROOM_STEM: RawBlockId = 125;
pub const ACACIA_LOG: RawBlockId = 126;
pub const ACACIA_LEAVES: RawBlockId = 127;
pub const JUNGLE_LOG: RawBlockId = 128;
pub const JUNGLE_LEAVES: RawBlockId = 129;
pub const BAMBOO: RawBlockId = 130;
pub const LILY_PAD: RawBlockId = 131;
pub const SWEET_BERRY_BUSH: RawBlockId = 132;
pub const ALLIUM: RawBlockId = 133;
pub const AZURE_BLUET: RawBlockId = 134;
pub const RED_TULIP: RawBlockId = 135;
pub const ORANGE_TULIP: RawBlockId = 136;
pub const WHITE_TULIP: RawBlockId = 137;
pub const PINK_TULIP: RawBlockId = 138;
pub const OXEYE_DAISY: RawBlockId = 139;
pub const CORNFLOWER: RawBlockId = 140;
pub const LILY_OF_THE_VALLEY: RawBlockId = 141;
pub const LILAC_LOWER: RawBlockId = 142;
pub const LILAC_UPPER: RawBlockId = 143;
pub const ROSE_BUSH_LOWER: RawBlockId = 144;
pub const ROSE_BUSH_UPPER: RawBlockId = 145;
pub const PEONY_LOWER: RawBlockId = 146;
pub const PEONY_UPPER: RawBlockId = 147;
pub const SUNFLOWER_LOWER: RawBlockId = 148;
pub const SUNFLOWER_UPPER: RawBlockId = 149;
pub const BLUE_ORCHID: RawBlockId = 150;
pub const BROWN_MUSHROOM: RawBlockId = 151;
pub const RED_MUSHROOM: RawBlockId = 152;
pub const BLUE_ICE: RawBlockId = 153;
pub const PUMPKIN: RawBlockId = 154;
pub const MELON: RawBlockId = 155;
pub const VINE_EAST: RawBlockId = 156;
pub const VINE: RawBlockId = VINE_EAST;
pub const TALL_GRASS_LOWER: RawBlockId = 157;
pub const TALL_GRASS_UPPER: RawBlockId = 158;
pub const VINE_UP: RawBlockId = 159;
pub const VINE_NORTH: RawBlockId = 160;
pub const VINE_SOUTH: RawBlockId = 161;
pub const VINE_WEST: RawBlockId = 162;
pub const COCOA_AGE0_NORTH: RawBlockId = 163;
pub const COCOA_AGE0_EAST: RawBlockId = 164;
pub const COCOA_AGE0_SOUTH: RawBlockId = 165;
pub const COCOA_AGE0_WEST: RawBlockId = 166;
pub const COCOA_AGE1_NORTH: RawBlockId = 167;
pub const COCOA_AGE1_EAST: RawBlockId = 168;
pub const COCOA_AGE1_SOUTH: RawBlockId = 169;
pub const COCOA_AGE1_WEST: RawBlockId = 170;
pub const COCOA_AGE2_NORTH: RawBlockId = 171;
pub const COCOA_AGE2_EAST: RawBlockId = 172;
pub const COCOA_AGE2_SOUTH: RawBlockId = 173;
pub const COCOA_AGE2_WEST: RawBlockId = 174;
pub const BAMBOO_TOP_SMALL: RawBlockId = 175;
pub const BAMBOO_TOP_LARGE: RawBlockId = 176;
pub const BAMBOO_FINAL_LARGE: RawBlockId = 177;
pub const MOSSY_COBBLESTONE: RawBlockId = 178;
pub const TUBE_CORAL: RawBlockId = 179;
pub const BRAIN_CORAL: RawBlockId = 180;
pub const BUBBLE_CORAL: RawBlockId = 181;
pub const FIRE_CORAL: RawBlockId = 182;
pub const HORN_CORAL: RawBlockId = 183;
pub const TUBE_CORAL_FAN: RawBlockId = 184;
pub const BRAIN_CORAL_FAN: RawBlockId = 185;
pub const BUBBLE_CORAL_FAN: RawBlockId = 186;
pub const FIRE_CORAL_FAN: RawBlockId = 187;
pub const HORN_CORAL_FAN: RawBlockId = 188;
pub const TUBE_CORAL_WALL_FAN_NORTH: RawBlockId = 189;
pub const TUBE_CORAL_WALL_FAN_EAST: RawBlockId = 190;
pub const TUBE_CORAL_WALL_FAN_SOUTH: RawBlockId = 191;
pub const TUBE_CORAL_WALL_FAN_WEST: RawBlockId = 192;
pub const BRAIN_CORAL_WALL_FAN_NORTH: RawBlockId = 193;
pub const BRAIN_CORAL_WALL_FAN_EAST: RawBlockId = 194;
pub const BRAIN_CORAL_WALL_FAN_SOUTH: RawBlockId = 195;
pub const BRAIN_CORAL_WALL_FAN_WEST: RawBlockId = 196;
pub const BUBBLE_CORAL_WALL_FAN_NORTH: RawBlockId = 197;
pub const BUBBLE_CORAL_WALL_FAN_EAST: RawBlockId = 198;
pub const BUBBLE_CORAL_WALL_FAN_SOUTH: RawBlockId = 199;
pub const BUBBLE_CORAL_WALL_FAN_WEST: RawBlockId = 200;
pub const FIRE_CORAL_WALL_FAN_NORTH: RawBlockId = 201;
pub const FIRE_CORAL_WALL_FAN_EAST: RawBlockId = 202;
pub const FIRE_CORAL_WALL_FAN_SOUTH: RawBlockId = 203;
pub const FIRE_CORAL_WALL_FAN_WEST: RawBlockId = 204;
pub const HORN_CORAL_WALL_FAN_NORTH: RawBlockId = 205;
pub const HORN_CORAL_WALL_FAN_EAST: RawBlockId = 206;
pub const HORN_CORAL_WALL_FAN_SOUTH: RawBlockId = 207;
pub const HORN_CORAL_WALL_FAN_WEST: RawBlockId = 208;
pub const OAK_PLANKS: RawBlockId = 209;
pub const SPRUCE_PLANKS: RawBlockId = 210;
pub const COBBLESTONE: RawBlockId = 211;
pub const STONE_BRICKS: RawBlockId = 212;
pub const HAY_BLOCK: RawBlockId = 213;

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
    pub const TALL_GRASS_LOWER: Self = Self(TALL_GRASS_LOWER);
    pub const TALL_GRASS_UPPER: Self = Self(TALL_GRASS_UPPER);
    pub const GLOW_LICHEN: Self = Self(GLOW_LICHEN);
    pub const CAVE_AIR: Self = Self(CAVE_AIR);
    pub const CLAY: Self = Self(CLAY);
    pub const DRIPSTONE_BLOCK: Self = Self(DRIPSTONE_BLOCK);
    pub const POINTED_DRIPSTONE: Self = Self(POINTED_DRIPSTONE);
    pub const BRICKS: Self = Self(BRICKS);
    pub const TORCH: Self = Self(TORCH);
    pub const CACTUS: Self = Self(CACTUS);
    pub const SUGAR_CANE: Self = Self(SUGAR_CANE);
    pub const SEAGRASS: Self = Self(SEAGRASS);
    pub const TALL_SEAGRASS_LOWER: Self = Self(TALL_SEAGRASS_LOWER);
    pub const TALL_SEAGRASS_UPPER: Self = Self(TALL_SEAGRASS_UPPER);
    pub const KELP: Self = Self(KELP);
    pub const KELP_PLANT: Self = Self(KELP_PLANT);
    pub const TUBE_CORAL_BLOCK: Self = Self(TUBE_CORAL_BLOCK);
    pub const BRAIN_CORAL_BLOCK: Self = Self(BRAIN_CORAL_BLOCK);
    pub const BUBBLE_CORAL_BLOCK: Self = Self(BUBBLE_CORAL_BLOCK);
    pub const FIRE_CORAL_BLOCK: Self = Self(FIRE_CORAL_BLOCK);
    pub const HORN_CORAL_BLOCK: Self = Self(HORN_CORAL_BLOCK);
    pub const SEA_PICKLE_1: Self = Self(SEA_PICKLE_1);
    pub const SEA_PICKLE_2: Self = Self(SEA_PICKLE_2);
    pub const SEA_PICKLE_3: Self = Self(SEA_PICKLE_3);
    pub const SEA_PICKLE_4: Self = Self(SEA_PICKLE_4);
    pub const DARK_OAK_LOG: Self = Self(DARK_OAK_LOG);
    pub const DARK_OAK_LEAVES: Self = Self(DARK_OAK_LEAVES);
    pub const BROWN_MUSHROOM_BLOCK: Self = Self(BROWN_MUSHROOM_BLOCK);
    pub const RED_MUSHROOM_BLOCK: Self = Self(RED_MUSHROOM_BLOCK);
    pub const MUSHROOM_STEM: Self = Self(MUSHROOM_STEM);
    pub const ACACIA_LOG: Self = Self(ACACIA_LOG);
    pub const ACACIA_LEAVES: Self = Self(ACACIA_LEAVES);
    pub const JUNGLE_LOG: Self = Self(JUNGLE_LOG);
    pub const JUNGLE_LEAVES: Self = Self(JUNGLE_LEAVES);
    pub const BAMBOO: Self = Self(BAMBOO);
    pub const LILY_PAD: Self = Self(LILY_PAD);
    pub const SWEET_BERRY_BUSH: Self = Self(SWEET_BERRY_BUSH);
    pub const ALLIUM: Self = Self(ALLIUM);
    pub const AZURE_BLUET: Self = Self(AZURE_BLUET);
    pub const RED_TULIP: Self = Self(RED_TULIP);
    pub const ORANGE_TULIP: Self = Self(ORANGE_TULIP);
    pub const WHITE_TULIP: Self = Self(WHITE_TULIP);
    pub const PINK_TULIP: Self = Self(PINK_TULIP);
    pub const OXEYE_DAISY: Self = Self(OXEYE_DAISY);
    pub const CORNFLOWER: Self = Self(CORNFLOWER);
    pub const LILY_OF_THE_VALLEY: Self = Self(LILY_OF_THE_VALLEY);
    pub const LILAC_LOWER: Self = Self(LILAC_LOWER);
    pub const LILAC_UPPER: Self = Self(LILAC_UPPER);
    pub const ROSE_BUSH_LOWER: Self = Self(ROSE_BUSH_LOWER);
    pub const ROSE_BUSH_UPPER: Self = Self(ROSE_BUSH_UPPER);
    pub const PEONY_LOWER: Self = Self(PEONY_LOWER);
    pub const PEONY_UPPER: Self = Self(PEONY_UPPER);
    pub const SUNFLOWER_LOWER: Self = Self(SUNFLOWER_LOWER);
    pub const SUNFLOWER_UPPER: Self = Self(SUNFLOWER_UPPER);
    pub const BLUE_ORCHID: Self = Self(BLUE_ORCHID);
    pub const BROWN_MUSHROOM: Self = Self(BROWN_MUSHROOM);
    pub const RED_MUSHROOM: Self = Self(RED_MUSHROOM);
    pub const BLUE_ICE: Self = Self(BLUE_ICE);
    pub const PUMPKIN: Self = Self(PUMPKIN);
    pub const MELON: Self = Self(MELON);
    pub const VINE: Self = Self(VINE);
    pub const VINE_UP: Self = Self(VINE_UP);
    pub const VINE_NORTH: Self = Self(VINE_NORTH);
    pub const VINE_SOUTH: Self = Self(VINE_SOUTH);
    pub const VINE_WEST: Self = Self(VINE_WEST);
    pub const COCOA_AGE0_NORTH: Self = Self(COCOA_AGE0_NORTH);
    pub const COCOA_AGE0_EAST: Self = Self(COCOA_AGE0_EAST);
    pub const COCOA_AGE0_SOUTH: Self = Self(COCOA_AGE0_SOUTH);
    pub const COCOA_AGE0_WEST: Self = Self(COCOA_AGE0_WEST);
    pub const COCOA_AGE1_NORTH: Self = Self(COCOA_AGE1_NORTH);
    pub const COCOA_AGE1_EAST: Self = Self(COCOA_AGE1_EAST);
    pub const COCOA_AGE1_SOUTH: Self = Self(COCOA_AGE1_SOUTH);
    pub const COCOA_AGE1_WEST: Self = Self(COCOA_AGE1_WEST);
    pub const COCOA_AGE2_NORTH: Self = Self(COCOA_AGE2_NORTH);
    pub const COCOA_AGE2_EAST: Self = Self(COCOA_AGE2_EAST);
    pub const COCOA_AGE2_SOUTH: Self = Self(COCOA_AGE2_SOUTH);
    pub const COCOA_AGE2_WEST: Self = Self(COCOA_AGE2_WEST);
    pub const BAMBOO_TOP_SMALL: Self = Self(BAMBOO_TOP_SMALL);
    pub const BAMBOO_TOP_LARGE: Self = Self(BAMBOO_TOP_LARGE);
    pub const BAMBOO_FINAL_LARGE: Self = Self(BAMBOO_FINAL_LARGE);
    pub const MOSSY_COBBLESTONE: Self = Self(MOSSY_COBBLESTONE);
    pub const TUBE_CORAL: Self = Self(TUBE_CORAL);
    pub const BRAIN_CORAL: Self = Self(BRAIN_CORAL);
    pub const BUBBLE_CORAL: Self = Self(BUBBLE_CORAL);
    pub const FIRE_CORAL: Self = Self(FIRE_CORAL);
    pub const HORN_CORAL: Self = Self(HORN_CORAL);
    pub const TUBE_CORAL_FAN: Self = Self(TUBE_CORAL_FAN);
    pub const BRAIN_CORAL_FAN: Self = Self(BRAIN_CORAL_FAN);
    pub const BUBBLE_CORAL_FAN: Self = Self(BUBBLE_CORAL_FAN);
    pub const FIRE_CORAL_FAN: Self = Self(FIRE_CORAL_FAN);
    pub const HORN_CORAL_FAN: Self = Self(HORN_CORAL_FAN);
    pub const TUBE_CORAL_WALL_FAN_NORTH: Self = Self(TUBE_CORAL_WALL_FAN_NORTH);
    pub const TUBE_CORAL_WALL_FAN_EAST: Self = Self(TUBE_CORAL_WALL_FAN_EAST);
    pub const TUBE_CORAL_WALL_FAN_SOUTH: Self = Self(TUBE_CORAL_WALL_FAN_SOUTH);
    pub const TUBE_CORAL_WALL_FAN_WEST: Self = Self(TUBE_CORAL_WALL_FAN_WEST);
    pub const BRAIN_CORAL_WALL_FAN_NORTH: Self = Self(BRAIN_CORAL_WALL_FAN_NORTH);
    pub const BRAIN_CORAL_WALL_FAN_EAST: Self = Self(BRAIN_CORAL_WALL_FAN_EAST);
    pub const BRAIN_CORAL_WALL_FAN_SOUTH: Self = Self(BRAIN_CORAL_WALL_FAN_SOUTH);
    pub const BRAIN_CORAL_WALL_FAN_WEST: Self = Self(BRAIN_CORAL_WALL_FAN_WEST);
    pub const BUBBLE_CORAL_WALL_FAN_NORTH: Self = Self(BUBBLE_CORAL_WALL_FAN_NORTH);
    pub const BUBBLE_CORAL_WALL_FAN_EAST: Self = Self(BUBBLE_CORAL_WALL_FAN_EAST);
    pub const BUBBLE_CORAL_WALL_FAN_SOUTH: Self = Self(BUBBLE_CORAL_WALL_FAN_SOUTH);
    pub const BUBBLE_CORAL_WALL_FAN_WEST: Self = Self(BUBBLE_CORAL_WALL_FAN_WEST);
    pub const FIRE_CORAL_WALL_FAN_NORTH: Self = Self(FIRE_CORAL_WALL_FAN_NORTH);
    pub const FIRE_CORAL_WALL_FAN_EAST: Self = Self(FIRE_CORAL_WALL_FAN_EAST);
    pub const FIRE_CORAL_WALL_FAN_SOUTH: Self = Self(FIRE_CORAL_WALL_FAN_SOUTH);
    pub const FIRE_CORAL_WALL_FAN_WEST: Self = Self(FIRE_CORAL_WALL_FAN_WEST);
    pub const HORN_CORAL_WALL_FAN_NORTH: Self = Self(HORN_CORAL_WALL_FAN_NORTH);
    pub const HORN_CORAL_WALL_FAN_EAST: Self = Self(HORN_CORAL_WALL_FAN_EAST);
    pub const HORN_CORAL_WALL_FAN_SOUTH: Self = Self(HORN_CORAL_WALL_FAN_SOUTH);
    pub const HORN_CORAL_WALL_FAN_WEST: Self = Self(HORN_CORAL_WALL_FAN_WEST);
    pub const OAK_PLANKS: Self = Self(OAK_PLANKS);
    pub const SPRUCE_PLANKS: Self = Self(SPRUCE_PLANKS);
    pub const COBBLESTONE: Self = Self(COBBLESTONE);
    pub const STONE_BRICKS: Self = Self(STONE_BRICKS);
    pub const HAY_BLOCK: Self = Self(HAY_BLOCK);

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
        is_water(self.0)
    }

    pub const fn name(self) -> &'static str {
        block_name(self.0)
    }
}

pub const fn is_air_like(block_id: RawBlockId) -> bool {
    block_id == AIR || block_id == CAVE_AIR
}

pub const fn material_blocks_motion(block_id: RawBlockId) -> bool {
    if has_fluid(block_id) {
        return false;
    }
    if is_vine(block_id)
        || is_cocoa(block_id)
        || is_bamboo(block_id)
        || is_coral_plant(block_id)
        || is_coral_fan(block_id)
        || is_coral_wall_fan(block_id)
    {
        return false;
    }

    !matches!(
        block_id,
        AIR | CAVE_AIR
            | SNOW
            | GRASS
            | FERN
            | DANDELION
            | POPPY
            | ALLIUM
            | AZURE_BLUET
            | RED_TULIP
            | ORANGE_TULIP
            | WHITE_TULIP
            | PINK_TULIP
            | OXEYE_DAISY
            | CORNFLOWER
            | LILY_OF_THE_VALLEY
            | DEAD_BUSH
            | LARGE_FERN_LOWER
            | LARGE_FERN_UPPER
            | TALL_GRASS_LOWER
            | TALL_GRASS_UPPER
            | LILAC_LOWER
            | LILAC_UPPER
            | ROSE_BUSH_LOWER
            | ROSE_BUSH_UPPER
            | PEONY_LOWER
            | PEONY_UPPER
            | SUNFLOWER_LOWER
            | SUNFLOWER_UPPER
            | BLUE_ORCHID
            | BROWN_MUSHROOM
            | RED_MUSHROOM
            | GLOW_LICHEN
            | POINTED_DRIPSTONE
            | TORCH
            | WALL_TORCH_NORTH
            | WALL_TORCH_EAST
            | WALL_TORCH_SOUTH
            | WALL_TORCH_WEST
            | SUGAR_CANE
            | SEAGRASS
            | TALL_SEAGRASS_LOWER
            | TALL_SEAGRASS_UPPER
            | KELP
            | KELP_PLANT
            | LILY_PAD
            | SWEET_BERRY_BUSH
            | SEA_PICKLE_1
            | SEA_PICKLE_2
            | SEA_PICKLE_3
            | SEA_PICKLE_4
    )
}

pub const fn block_light_opacity(block_id: RawBlockId) -> u8 {
    if is_leaves(block_id) {
        1
    } else if has_fluid(block_id) {
        1
    } else if material_blocks_motion(block_id) {
        15
    } else {
        0
    }
}

pub const fn block_light_emission(block_id: RawBlockId) -> u8 {
    if is_lava(block_id) {
        15
    } else {
        match block_id {
            MAGMA_BLOCK => 3,
            GLOW_LICHEN => 7,
            BROWN_MUSHROOM => 1,
            TORCH | WALL_TORCH_NORTH | WALL_TORCH_EAST | WALL_TORCH_SOUTH | WALL_TORCH_WEST => 14,
            SEA_PICKLE_1 => 6,
            SEA_PICKLE_2 => 9,
            SEA_PICKLE_3 => 12,
            SEA_PICKLE_4 => 15,
            _ => 0,
        }
    }
}

pub const fn is_coral_block(block_id: RawBlockId) -> bool {
    matches!(
        block_id,
        TUBE_CORAL_BLOCK
            | BRAIN_CORAL_BLOCK
            | BUBBLE_CORAL_BLOCK
            | FIRE_CORAL_BLOCK
            | HORN_CORAL_BLOCK
    )
}

pub const fn is_coral_plant(block_id: RawBlockId) -> bool {
    matches!(
        block_id,
        TUBE_CORAL | BRAIN_CORAL | BUBBLE_CORAL | FIRE_CORAL | HORN_CORAL
    )
}

pub const fn is_coral_fan(block_id: RawBlockId) -> bool {
    matches!(
        block_id,
        TUBE_CORAL_FAN | BRAIN_CORAL_FAN | BUBBLE_CORAL_FAN | FIRE_CORAL_FAN | HORN_CORAL_FAN
    )
}

pub const fn is_coral_wall_fan(block_id: RawBlockId) -> bool {
    matches!(
        block_id,
        TUBE_CORAL_WALL_FAN_NORTH
            | TUBE_CORAL_WALL_FAN_EAST
            | TUBE_CORAL_WALL_FAN_SOUTH
            | TUBE_CORAL_WALL_FAN_WEST
            | BRAIN_CORAL_WALL_FAN_NORTH
            | BRAIN_CORAL_WALL_FAN_EAST
            | BRAIN_CORAL_WALL_FAN_SOUTH
            | BRAIN_CORAL_WALL_FAN_WEST
            | BUBBLE_CORAL_WALL_FAN_NORTH
            | BUBBLE_CORAL_WALL_FAN_EAST
            | BUBBLE_CORAL_WALL_FAN_SOUTH
            | BUBBLE_CORAL_WALL_FAN_WEST
            | FIRE_CORAL_WALL_FAN_NORTH
            | FIRE_CORAL_WALL_FAN_EAST
            | FIRE_CORAL_WALL_FAN_SOUTH
            | FIRE_CORAL_WALL_FAN_WEST
            | HORN_CORAL_WALL_FAN_NORTH
            | HORN_CORAL_WALL_FAN_EAST
            | HORN_CORAL_WALL_FAN_SOUTH
            | HORN_CORAL_WALL_FAN_WEST
    )
}

pub const fn is_coral_plant_or_fan(block_id: RawBlockId) -> bool {
    is_coral_plant(block_id) || is_coral_fan(block_id)
}

pub const fn is_sea_pickle(block_id: RawBlockId) -> bool {
    matches!(
        block_id,
        SEA_PICKLE_1 | SEA_PICKLE_2 | SEA_PICKLE_3 | SEA_PICKLE_4
    )
}

pub const fn is_vine(block_id: RawBlockId) -> bool {
    matches!(
        block_id,
        VINE_EAST | VINE_UP | VINE_NORTH | VINE_SOUTH | VINE_WEST
    )
}

pub const fn is_cocoa(block_id: RawBlockId) -> bool {
    matches!(
        block_id,
        COCOA_AGE0_NORTH
            | COCOA_AGE0_EAST
            | COCOA_AGE0_SOUTH
            | COCOA_AGE0_WEST
            | COCOA_AGE1_NORTH
            | COCOA_AGE1_EAST
            | COCOA_AGE1_SOUTH
            | COCOA_AGE1_WEST
            | COCOA_AGE2_NORTH
            | COCOA_AGE2_EAST
            | COCOA_AGE2_SOUTH
            | COCOA_AGE2_WEST
    )
}

pub const fn is_bamboo(block_id: RawBlockId) -> bool {
    matches!(
        block_id,
        BAMBOO | BAMBOO_TOP_SMALL | BAMBOO_TOP_LARGE | BAMBOO_FINAL_LARGE
    )
}

pub const fn base_block_id(block_id: RawBlockId) -> RawBlockId {
    match block_id {
        OAK_LOG_X | OAK_LOG_Z => OAK_LOG,
        BIRCH_LOG_X | BIRCH_LOG_Z => BIRCH_LOG,
        SPRUCE_LOG_X | SPRUCE_LOG_Z => SPRUCE_LOG,
        DEEPSLATE_X | DEEPSLATE_Z => DEEPSLATE,
        WALL_TORCH_NORTH | WALL_TORCH_EAST | WALL_TORCH_SOUTH | WALL_TORCH_WEST => TORCH,
        VINE_UP | VINE_NORTH | VINE_SOUTH | VINE_WEST => VINE,
        COCOA_AGE0_EAST | COCOA_AGE0_SOUTH | COCOA_AGE0_WEST | COCOA_AGE1_NORTH
        | COCOA_AGE1_EAST | COCOA_AGE1_SOUTH | COCOA_AGE1_WEST | COCOA_AGE2_NORTH
        | COCOA_AGE2_EAST | COCOA_AGE2_SOUTH | COCOA_AGE2_WEST => COCOA_AGE0_NORTH,
        BAMBOO_TOP_SMALL | BAMBOO_TOP_LARGE | BAMBOO_FINAL_LARGE => BAMBOO,
        TUBE_CORAL_WALL_FAN_EAST | TUBE_CORAL_WALL_FAN_SOUTH | TUBE_CORAL_WALL_FAN_WEST => {
            TUBE_CORAL_WALL_FAN_NORTH
        }
        BRAIN_CORAL_WALL_FAN_EAST | BRAIN_CORAL_WALL_FAN_SOUTH | BRAIN_CORAL_WALL_FAN_WEST => {
            BRAIN_CORAL_WALL_FAN_NORTH
        }
        BUBBLE_CORAL_WALL_FAN_EAST | BUBBLE_CORAL_WALL_FAN_SOUTH | BUBBLE_CORAL_WALL_FAN_WEST => {
            BUBBLE_CORAL_WALL_FAN_NORTH
        }
        FIRE_CORAL_WALL_FAN_EAST | FIRE_CORAL_WALL_FAN_SOUTH | FIRE_CORAL_WALL_FAN_WEST => {
            FIRE_CORAL_WALL_FAN_NORTH
        }
        HORN_CORAL_WALL_FAN_EAST | HORN_CORAL_WALL_FAN_SOUTH | HORN_CORAL_WALL_FAN_WEST => {
            HORN_CORAL_WALL_FAN_NORTH
        }
        _ => block_id,
    }
}

pub const fn is_water(block_id: RawBlockId) -> bool {
    block_id == WATER || (block_id >= WATER_LEVEL_1 && block_id <= WATER_LEVEL_8)
}

pub const fn is_lava(block_id: RawBlockId) -> bool {
    block_id == LAVA || (block_id >= LAVA_LEVEL_1 && block_id <= LAVA_LEVEL_8)
}

pub const fn has_fluid(block_id: RawBlockId) -> bool {
    is_water(block_id) || is_lava(block_id) || holds_source_water(block_id)
}

/// Blocks that report a full water source from their fluid state without being
/// raw water. Mirrors vanilla `getFluidState`: `SeagrassBlock`,
/// `TallSeagrassBlock`, `KelpBlock`, and `KelpPlantBlock` always return source
/// water, and the waterlogged `SimpleWaterloggedBlock` sea life (sea pickles,
/// coral plants/fans/wall fans) returns source water because worldgen only ever
/// places them with `waterlogged=true`. Coral *blocks* are full solid cubes that
/// are never waterlogged and are intentionally excluded.
pub const fn holds_source_water(block_id: RawBlockId) -> bool {
    matches!(
        block_id,
        SEAGRASS
            | TALL_SEAGRASS_LOWER
            | TALL_SEAGRASS_UPPER
            | KELP
            | KELP_PLANT
            | SEA_PICKLE_1
            | SEA_PICKLE_2
            | SEA_PICKLE_3
            | SEA_PICKLE_4
            | TUBE_CORAL
            | BRAIN_CORAL
            | BUBBLE_CORAL
            | FIRE_CORAL
            | HORN_CORAL
            | TUBE_CORAL_FAN
            | BRAIN_CORAL_FAN
            | BUBBLE_CORAL_FAN
            | FIRE_CORAL_FAN
            | HORN_CORAL_FAN
    ) || (block_id >= TUBE_CORAL_WALL_FAN_NORTH && block_id <= HORN_CORAL_WALL_FAN_WEST)
}

pub const fn fluid_level(block_id: RawBlockId) -> Option<u8> {
    match block_id {
        WATER | LAVA => Some(0),
        WATER_LEVEL_1 | LAVA_LEVEL_1 => Some(1),
        WATER_LEVEL_2 | LAVA_LEVEL_2 => Some(2),
        WATER_LEVEL_3 | LAVA_LEVEL_3 => Some(3),
        WATER_LEVEL_4 | LAVA_LEVEL_4 => Some(4),
        WATER_LEVEL_5 | LAVA_LEVEL_5 => Some(5),
        WATER_LEVEL_6 | LAVA_LEVEL_6 => Some(6),
        WATER_LEVEL_7 | LAVA_LEVEL_7 => Some(7),
        WATER_LEVEL_8 | LAVA_LEVEL_8 => Some(8),
        _ => None,
    }
}

pub const fn water_block_for_level(level: u8) -> Option<RawBlockId> {
    match level {
        0 => Some(WATER),
        1 => Some(WATER_LEVEL_1),
        2 => Some(WATER_LEVEL_2),
        3 => Some(WATER_LEVEL_3),
        4 => Some(WATER_LEVEL_4),
        5 => Some(WATER_LEVEL_5),
        6 => Some(WATER_LEVEL_6),
        7 => Some(WATER_LEVEL_7),
        8 => Some(WATER_LEVEL_8),
        _ => None,
    }
}

pub const fn lava_block_for_level(level: u8) -> Option<RawBlockId> {
    match level {
        0 => Some(LAVA),
        1 => Some(LAVA_LEVEL_1),
        2 => Some(LAVA_LEVEL_2),
        3 => Some(LAVA_LEVEL_3),
        4 => Some(LAVA_LEVEL_4),
        5 => Some(LAVA_LEVEL_5),
        6 => Some(LAVA_LEVEL_6),
        7 => Some(LAVA_LEVEL_7),
        8 => Some(LAVA_LEVEL_8),
        _ => None,
    }
}

pub const fn is_leaves(block_id: RawBlockId) -> bool {
    matches!(
        block_id,
        OAK_LEAVES | BIRCH_LEAVES | SPRUCE_LEAVES | DARK_OAK_LEAVES | ACACIA_LEAVES | JUNGLE_LEAVES
    )
}

pub const fn generated_block_state_id(block_id: RawBlockId) -> BlockStateId {
    BlockStateId(block_id as u32)
}

pub const fn block_name(block_id: RawBlockId) -> &'static str {
    if is_water(block_id) {
        return "minecraft:water";
    }
    if is_lava(block_id) {
        return "minecraft:lava";
    }
    if is_vine(block_id) {
        return "minecraft:vine";
    }
    if is_cocoa(block_id) {
        return "minecraft:cocoa";
    }
    if is_bamboo(block_id) {
        return "minecraft:bamboo";
    }

    match block_id {
        AIR => "minecraft:air",
        STONE => "minecraft:stone",
        BEDROCK => "minecraft:bedrock",
        GRASS_BLOCK => "minecraft:grass_block",
        DIRT => "minecraft:dirt",
        SAND => "minecraft:sand",
        GRAVEL => "minecraft:gravel",
        SNOW => "minecraft:snow",
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
        BLUE_ICE => "minecraft:blue_ice",
        MELON => "minecraft:melon",
        OBSIDIAN => "minecraft:obsidian",
        MAGMA_BLOCK => "minecraft:magma_block",
        RED_SAND => "minecraft:red_sand",
        ICE => "minecraft:ice",
        SNOW_BLOCK => "minecraft:snow_block",
        OAK_LOG | OAK_LOG_X | OAK_LOG_Z => "minecraft:oak_log",
        OAK_LEAVES => "minecraft:oak_leaves",
        GRASS => "minecraft:grass",
        DANDELION => "minecraft:dandelion",
        POPPY => "minecraft:poppy",
        BIRCH_LOG | BIRCH_LOG_X | BIRCH_LOG_Z => "minecraft:birch_log",
        BIRCH_LEAVES => "minecraft:birch_leaves",
        SPRUCE_LOG | SPRUCE_LOG_X | SPRUCE_LOG_Z => "minecraft:spruce_log",
        SPRUCE_LEAVES => "minecraft:spruce_leaves",
        DARK_OAK_LOG => "minecraft:dark_oak_log",
        DARK_OAK_LEAVES => "minecraft:dark_oak_leaves",
        BROWN_MUSHROOM_BLOCK => "minecraft:brown_mushroom_block",
        RED_MUSHROOM_BLOCK => "minecraft:red_mushroom_block",
        MUSHROOM_STEM => "minecraft:mushroom_stem",
        ACACIA_LOG => "minecraft:acacia_log",
        ACACIA_LEAVES => "minecraft:acacia_leaves",
        JUNGLE_LOG => "minecraft:jungle_log",
        JUNGLE_LEAVES => "minecraft:jungle_leaves",
        LILY_PAD => "minecraft:lily_pad",
        SWEET_BERRY_BUSH => "minecraft:sweet_berry_bush",
        ALLIUM => "minecraft:allium",
        AZURE_BLUET => "minecraft:azure_bluet",
        RED_TULIP => "minecraft:red_tulip",
        ORANGE_TULIP => "minecraft:orange_tulip",
        WHITE_TULIP => "minecraft:white_tulip",
        PINK_TULIP => "minecraft:pink_tulip",
        OXEYE_DAISY => "minecraft:oxeye_daisy",
        CORNFLOWER => "minecraft:cornflower",
        LILY_OF_THE_VALLEY => "minecraft:lily_of_the_valley",
        LILAC_LOWER | LILAC_UPPER => "minecraft:lilac",
        ROSE_BUSH_LOWER | ROSE_BUSH_UPPER => "minecraft:rose_bush",
        PEONY_LOWER | PEONY_UPPER => "minecraft:peony",
        SUNFLOWER_LOWER | SUNFLOWER_UPPER => "minecraft:sunflower",
        BLUE_ORCHID => "minecraft:blue_orchid",
        BROWN_MUSHROOM => "minecraft:brown_mushroom",
        RED_MUSHROOM => "minecraft:red_mushroom",
        PUMPKIN => "minecraft:pumpkin",
        FERN => "minecraft:fern",
        DEAD_BUSH => "minecraft:dead_bush",
        TUFF => "minecraft:tuff",
        DEEPSLATE | DEEPSLATE_X | DEEPSLATE_Z => "minecraft:deepslate",
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
        TALL_GRASS_LOWER | TALL_GRASS_UPPER => "minecraft:tall_grass",
        GLOW_LICHEN => "minecraft:glow_lichen",
        CAVE_AIR => "minecraft:cave_air",
        CLAY => "minecraft:clay",
        DRIPSTONE_BLOCK => "minecraft:dripstone_block",
        POINTED_DRIPSTONE => "minecraft:pointed_dripstone",
        BRICKS => "minecraft:bricks",
        TORCH => "minecraft:torch",
        WALL_TORCH_NORTH | WALL_TORCH_EAST | WALL_TORCH_SOUTH | WALL_TORCH_WEST => {
            "minecraft:wall_torch"
        }
        CACTUS => "minecraft:cactus",
        SUGAR_CANE => "minecraft:sugar_cane",
        SEAGRASS => "minecraft:seagrass",
        TALL_SEAGRASS_LOWER | TALL_SEAGRASS_UPPER => "minecraft:tall_seagrass",
        KELP => "minecraft:kelp",
        KELP_PLANT => "minecraft:kelp_plant",
        TUBE_CORAL_BLOCK => "minecraft:tube_coral_block",
        BRAIN_CORAL_BLOCK => "minecraft:brain_coral_block",
        BUBBLE_CORAL_BLOCK => "minecraft:bubble_coral_block",
        FIRE_CORAL_BLOCK => "minecraft:fire_coral_block",
        HORN_CORAL_BLOCK => "minecraft:horn_coral_block",
        TUBE_CORAL => "minecraft:tube_coral",
        BRAIN_CORAL => "minecraft:brain_coral",
        BUBBLE_CORAL => "minecraft:bubble_coral",
        FIRE_CORAL => "minecraft:fire_coral",
        HORN_CORAL => "minecraft:horn_coral",
        TUBE_CORAL_FAN => "minecraft:tube_coral_fan",
        BRAIN_CORAL_FAN => "minecraft:brain_coral_fan",
        BUBBLE_CORAL_FAN => "minecraft:bubble_coral_fan",
        FIRE_CORAL_FAN => "minecraft:fire_coral_fan",
        HORN_CORAL_FAN => "minecraft:horn_coral_fan",
        TUBE_CORAL_WALL_FAN_NORTH
        | TUBE_CORAL_WALL_FAN_EAST
        | TUBE_CORAL_WALL_FAN_SOUTH
        | TUBE_CORAL_WALL_FAN_WEST => "minecraft:tube_coral_wall_fan",
        BRAIN_CORAL_WALL_FAN_NORTH
        | BRAIN_CORAL_WALL_FAN_EAST
        | BRAIN_CORAL_WALL_FAN_SOUTH
        | BRAIN_CORAL_WALL_FAN_WEST => "minecraft:brain_coral_wall_fan",
        BUBBLE_CORAL_WALL_FAN_NORTH
        | BUBBLE_CORAL_WALL_FAN_EAST
        | BUBBLE_CORAL_WALL_FAN_SOUTH
        | BUBBLE_CORAL_WALL_FAN_WEST => "minecraft:bubble_coral_wall_fan",
        FIRE_CORAL_WALL_FAN_NORTH
        | FIRE_CORAL_WALL_FAN_EAST
        | FIRE_CORAL_WALL_FAN_SOUTH
        | FIRE_CORAL_WALL_FAN_WEST => "minecraft:fire_coral_wall_fan",
        HORN_CORAL_WALL_FAN_NORTH
        | HORN_CORAL_WALL_FAN_EAST
        | HORN_CORAL_WALL_FAN_SOUTH
        | HORN_CORAL_WALL_FAN_WEST => "minecraft:horn_coral_wall_fan",
        SEA_PICKLE_1 | SEA_PICKLE_2 | SEA_PICKLE_3 | SEA_PICKLE_4 => "minecraft:sea_pickle",
        MOSSY_COBBLESTONE => "minecraft:mossy_cobblestone",
        OAK_PLANKS => "minecraft:oak_planks",
        SPRUCE_PLANKS => "minecraft:spruce_planks",
        COBBLESTONE => "minecraft:cobblestone",
        STONE_BRICKS => "minecraft:stone_bricks",
        HAY_BLOCK => "minecraft:hay_block",
        _ => "minecraft:unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_light_facts_cover_current_emitting_blocks() {
        assert_eq!(block_light_emission(LAVA), 15);
        assert_eq!(block_light_emission(LAVA_LEVEL_8), 15);
        assert_eq!(block_light_emission(MAGMA_BLOCK), 3);
        assert_eq!(block_light_emission(GLOW_LICHEN), 7);
        assert_eq!(block_light_emission(BROWN_MUSHROOM), 1);
        assert_eq!(block_light_emission(RED_MUSHROOM), 0);
        assert_eq!(block_light_emission(TORCH), 14);
        assert_eq!(block_light_emission(WALL_TORCH_NORTH), 14);
        assert_eq!(block_light_emission(SEA_PICKLE_1), 6);
        assert_eq!(block_light_emission(SEA_PICKLE_4), 15);
        assert_eq!(block_light_emission(STONE), 0);
    }

    #[test]
    fn block_light_opacity_tracks_current_block_facts() {
        assert_eq!(block_light_opacity(STONE), 15);
        assert_eq!(block_light_opacity(OAK_LEAVES), 1);
        assert_eq!(block_light_opacity(BIRCH_LEAVES), 1);
        assert_eq!(block_light_opacity(SPRUCE_LEAVES), 1);
        assert_eq!(block_light_opacity(DARK_OAK_LEAVES), 1);
        assert_eq!(block_light_opacity(ACACIA_LEAVES), 1);
        assert_eq!(block_light_opacity(JUNGLE_LEAVES), 1);
        assert_eq!(block_light_opacity(AIR), 0);
        assert_eq!(block_light_opacity(CAVE_AIR), 0);
        assert_eq!(block_light_opacity(WATER), 1);
        assert_eq!(block_light_opacity(LAVA), 1);
        assert_eq!(block_light_opacity(GLOW_LICHEN), 0);
        assert_eq!(block_light_opacity(DRIPSTONE_BLOCK), 15);
        assert_eq!(block_light_opacity(POINTED_DRIPSTONE), 0);
        assert_eq!(block_light_opacity(TORCH), 0);
        assert_eq!(block_light_opacity(WALL_TORCH_EAST), 0);
        assert_eq!(block_light_opacity(CACTUS), 15);
        assert_eq!(block_light_opacity(PUMPKIN), 15);
        assert_eq!(block_light_opacity(MELON), 15);
        assert_eq!(block_light_opacity(MOSSY_COBBLESTONE), 15);
        assert_eq!(block_light_opacity(VINE), 0);
        assert_eq!(block_light_opacity(VINE_NORTH), 0);
        assert_eq!(block_light_opacity(VINE_WEST), 0);
        assert_eq!(block_light_opacity(COCOA_AGE2_SOUTH), 0);
        assert_eq!(block_light_opacity(TALL_GRASS_LOWER), 0);
        assert_eq!(block_light_opacity(TALL_GRASS_UPPER), 0);
        assert_eq!(block_light_opacity(SUGAR_CANE), 0);
        // Seagrass/kelp and waterlogged coral hold a source water fluid state, so
        // vanilla getLightBlock returns 1 (propagatesSkylightDown is false because
        // the fluid state is non-empty), just like water itself.
        assert_eq!(block_light_opacity(SEAGRASS), 1);
        assert_eq!(block_light_opacity(TALL_SEAGRASS_LOWER), 1);
        assert_eq!(block_light_opacity(KELP), 1);
        assert_eq!(block_light_opacity(KELP_PLANT), 1);
        assert_eq!(block_light_opacity(TUBE_CORAL), 1);
        assert_eq!(block_light_opacity(HORN_CORAL_FAN), 1);
        assert_eq!(block_light_opacity(BRAIN_CORAL_WALL_FAN_WEST), 1);
        assert_eq!(block_light_opacity(BAMBOO), 0);
        assert_eq!(block_light_opacity(BAMBOO_TOP_SMALL), 0);
        assert_eq!(block_light_opacity(BAMBOO_TOP_LARGE), 0);
        assert_eq!(block_light_opacity(BAMBOO_FINAL_LARGE), 0);
        assert_eq!(block_light_opacity(LILY_PAD), 0);
        assert_eq!(block_light_opacity(SWEET_BERRY_BUSH), 0);
        assert_eq!(block_light_opacity(ALLIUM), 0);
        assert_eq!(block_light_opacity(AZURE_BLUET), 0);
        assert_eq!(block_light_opacity(RED_TULIP), 0);
        assert_eq!(block_light_opacity(ORANGE_TULIP), 0);
        assert_eq!(block_light_opacity(WHITE_TULIP), 0);
        assert_eq!(block_light_opacity(PINK_TULIP), 0);
        assert_eq!(block_light_opacity(OXEYE_DAISY), 0);
        assert_eq!(block_light_opacity(CORNFLOWER), 0);
        assert_eq!(block_light_opacity(LILY_OF_THE_VALLEY), 0);
        assert_eq!(block_light_opacity(LILAC_LOWER), 0);
        assert_eq!(block_light_opacity(LILAC_UPPER), 0);
        assert_eq!(block_light_opacity(ROSE_BUSH_LOWER), 0);
        assert_eq!(block_light_opacity(ROSE_BUSH_UPPER), 0);
        assert_eq!(block_light_opacity(PEONY_LOWER), 0);
        assert_eq!(block_light_opacity(PEONY_UPPER), 0);
        assert_eq!(block_light_opacity(SUNFLOWER_LOWER), 0);
        assert_eq!(block_light_opacity(SUNFLOWER_UPPER), 0);
        assert_eq!(block_light_opacity(BLUE_ORCHID), 0);
        assert_eq!(block_light_opacity(BROWN_MUSHROOM), 0);
        assert_eq!(block_light_opacity(RED_MUSHROOM), 0);
        assert_eq!(block_light_opacity(SEA_PICKLE_1), 1);
        assert_eq!(block_light_opacity(TUBE_CORAL_BLOCK), 15);
        assert_eq!(block_light_opacity(BLUE_ICE), 15);
        assert_eq!(block_light_opacity(DARK_OAK_LOG), 15);
        assert_eq!(block_light_opacity(ACACIA_LOG), 15);
        assert_eq!(block_light_opacity(BROWN_MUSHROOM_BLOCK), 15);
    }

    #[test]
    fn torch_wall_variants_share_torch_base_block() {
        assert_eq!(base_block_id(TORCH), TORCH);
        assert_eq!(base_block_id(WALL_TORCH_NORTH), TORCH);
        assert_eq!(base_block_id(WALL_TORCH_EAST), TORCH);
        assert_eq!(base_block_id(WALL_TORCH_SOUTH), TORCH);
        assert_eq!(base_block_id(WALL_TORCH_WEST), TORCH);
        assert_eq!(base_block_id(VINE), VINE);
        assert_eq!(base_block_id(VINE_UP), VINE);
        assert_eq!(base_block_id(VINE_NORTH), VINE);
        assert_eq!(base_block_id(COCOA_AGE0_NORTH), COCOA_AGE0_NORTH);
        assert_eq!(base_block_id(COCOA_AGE2_WEST), COCOA_AGE0_NORTH);
        assert_eq!(base_block_id(BAMBOO), BAMBOO);
        assert_eq!(base_block_id(BAMBOO_TOP_SMALL), BAMBOO);
        assert_eq!(base_block_id(BAMBOO_TOP_LARGE), BAMBOO);
        assert_eq!(base_block_id(BAMBOO_FINAL_LARGE), BAMBOO);
        assert_eq!(
            base_block_id(TUBE_CORAL_WALL_FAN_EAST),
            TUBE_CORAL_WALL_FAN_NORTH
        );
        assert_eq!(
            base_block_id(HORN_CORAL_WALL_FAN_WEST),
            HORN_CORAL_WALL_FAN_NORTH
        );
        assert_eq!(block_name(TORCH), "minecraft:torch");
        assert_eq!(block_name(WALL_TORCH_WEST), "minecraft:wall_torch");
        assert_eq!(block_name(CACTUS), "minecraft:cactus");
        assert_eq!(block_name(SUGAR_CANE), "minecraft:sugar_cane");
        assert_eq!(block_name(SEAGRASS), "minecraft:seagrass");
        assert_eq!(block_name(TALL_SEAGRASS_UPPER), "minecraft:tall_seagrass");
        assert_eq!(block_name(KELP), "minecraft:kelp");
        assert_eq!(block_name(KELP_PLANT), "minecraft:kelp_plant");
        assert_eq!(block_name(BLUE_ICE), "minecraft:blue_ice");
        assert_eq!(block_name(PUMPKIN), "minecraft:pumpkin");
        assert_eq!(block_name(MELON), "minecraft:melon");
        assert_eq!(block_name(VINE), "minecraft:vine");
        assert_eq!(block_name(VINE_UP), "minecraft:vine");
        assert_eq!(block_name(VINE_WEST), "minecraft:vine");
        assert_eq!(block_name(COCOA_AGE0_NORTH), "minecraft:cocoa");
        assert_eq!(block_name(COCOA_AGE2_WEST), "minecraft:cocoa");
        assert_eq!(block_name(TALL_GRASS_UPPER), "minecraft:tall_grass");
        assert_eq!(block_name(TUBE_CORAL_BLOCK), "minecraft:tube_coral_block");
        assert_eq!(block_name(HORN_CORAL_BLOCK), "minecraft:horn_coral_block");
        assert_eq!(block_name(TUBE_CORAL), "minecraft:tube_coral");
        assert_eq!(block_name(FIRE_CORAL_FAN), "minecraft:fire_coral_fan");
        assert_eq!(
            block_name(BRAIN_CORAL_WALL_FAN_SOUTH),
            "minecraft:brain_coral_wall_fan"
        );
        assert_eq!(block_name(SEA_PICKLE_4), "minecraft:sea_pickle");
        assert_eq!(block_name(DARK_OAK_LOG), "minecraft:dark_oak_log");
        assert_eq!(block_name(DARK_OAK_LEAVES), "minecraft:dark_oak_leaves");
        assert_eq!(
            block_name(BROWN_MUSHROOM_BLOCK),
            "minecraft:brown_mushroom_block"
        );
        assert_eq!(
            block_name(RED_MUSHROOM_BLOCK),
            "minecraft:red_mushroom_block"
        );
        assert_eq!(block_name(MUSHROOM_STEM), "minecraft:mushroom_stem");
        assert_eq!(block_name(ACACIA_LOG), "minecraft:acacia_log");
        assert_eq!(block_name(ACACIA_LEAVES), "minecraft:acacia_leaves");
        assert_eq!(block_name(JUNGLE_LOG), "minecraft:jungle_log");
        assert_eq!(block_name(JUNGLE_LEAVES), "minecraft:jungle_leaves");
        assert_eq!(block_name(BAMBOO), "minecraft:bamboo");
        assert_eq!(block_name(BAMBOO_TOP_SMALL), "minecraft:bamboo");
        assert_eq!(block_name(BAMBOO_TOP_LARGE), "minecraft:bamboo");
        assert_eq!(block_name(BAMBOO_FINAL_LARGE), "minecraft:bamboo");
        assert_eq!(block_name(LILY_PAD), "minecraft:lily_pad");
        assert_eq!(block_name(SWEET_BERRY_BUSH), "minecraft:sweet_berry_bush");
        assert_eq!(block_name(ALLIUM), "minecraft:allium");
        assert_eq!(block_name(AZURE_BLUET), "minecraft:azure_bluet");
        assert_eq!(block_name(RED_TULIP), "minecraft:red_tulip");
        assert_eq!(block_name(ORANGE_TULIP), "minecraft:orange_tulip");
        assert_eq!(block_name(WHITE_TULIP), "minecraft:white_tulip");
        assert_eq!(block_name(PINK_TULIP), "minecraft:pink_tulip");
        assert_eq!(block_name(OXEYE_DAISY), "minecraft:oxeye_daisy");
        assert_eq!(block_name(CORNFLOWER), "minecraft:cornflower");
        assert_eq!(
            block_name(LILY_OF_THE_VALLEY),
            "minecraft:lily_of_the_valley"
        );
        assert_eq!(block_name(LILAC_LOWER), "minecraft:lilac");
        assert_eq!(block_name(LILAC_UPPER), "minecraft:lilac");
        assert_eq!(block_name(ROSE_BUSH_LOWER), "minecraft:rose_bush");
        assert_eq!(block_name(ROSE_BUSH_UPPER), "minecraft:rose_bush");
        assert_eq!(block_name(PEONY_LOWER), "minecraft:peony");
        assert_eq!(block_name(PEONY_UPPER), "minecraft:peony");
        assert_eq!(block_name(SUNFLOWER_LOWER), "minecraft:sunflower");
        assert_eq!(block_name(SUNFLOWER_UPPER), "minecraft:sunflower");
        assert_eq!(block_name(BLUE_ORCHID), "minecraft:blue_orchid");
        assert_eq!(block_name(BROWN_MUSHROOM), "minecraft:brown_mushroom");
        assert_eq!(block_name(RED_MUSHROOM), "minecraft:red_mushroom");
    }
}
