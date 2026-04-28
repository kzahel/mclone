import { BlockPos } from "../../core/block-pos";
import { Registry } from "../../core/registry";
import { ResourceLocation } from "../../core/resource-location";
import { registerGeneratedRenderBlocks } from "../../world/level/generated-render-blocks";
import { BoundingBox } from "../../world/level/levelgen/structure/bounding-box";
import type { Block } from "../../world/level/block/block";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import { ChunkBlockId } from "../../worldgen/chunk/chunk-block-buffer";
import { ChunkPos } from "../../core/chunk-pos";
import { createWorldGeneratorForPreset } from "../../worldgen/levelgen/world-generator-factory";
import type { WorldGenerator } from "../../worldgen/levelgen/world-generator";
import { NoneFeatureConfiguration } from "../../worldgen/levelgen/feature/configurations/none-feature-configuration";
import { OVERWORLD_FOSSIL_CONFIGURATION } from "../../worldgen/levelgen/feature/fossil-feature-defaults";
import { Features } from "../../worldgen/levelgen/feature/features";
import { WorldgenRandom } from "../../worldgen/prng/worldgen-random";
import { BuriedTreasurePiece } from "../../worldgen/levelgen/structure/buried-treasure-pieces";
import type { OpenWorldRequest, WorldEngineLightingMode, WorldEngineLiquidSimulationMode } from "../protocol/world-messages";
import type { LightingService } from "../lighting/lighting-protocol";
import type { WorldStorage } from "../storage/world-storage";
import { GeneratedWorldHost } from "./generated-world-host";

function getRequiredState(location: string): BlockState {
  const block = Registry.BLOCK.get(new ResourceLocation(location)) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

function applySmokeWorldMutations(
  level: WorldGenLevel,
  airState: BlockState,
  waterState: BlockState,
  stoneState: BlockState,
  sandState: BlockState,
  sandstoneState: BlockState,
  generator: WorldGenerator,
  seed: bigint,
): void {
  for (let z = 35; z <= 39; z++) {
    for (let x = 42; x <= 47; x++) {
      level.setBlock(new BlockPos(x, 84, z), waterState);
    }
  }

  const center = new BlockPos(168, 192, 168);
  for (let offsetX = -6; offsetX <= 6; offsetX++) {
    for (let offsetZ = -6; offsetZ <= 6; offsetZ++) {
      level.setBlock(center.offset(offsetX, -2, offsetZ), sandstoneState, 2);
      level.setBlock(center.offset(offsetX, -1, offsetZ), sandstoneState, 2);
      level.setBlock(center.offset(offsetX, 0, offsetZ), sandState, 2);
    }
  }

  Features.DESERT_WELL.configured(NoneFeatureConfiguration.INSTANCE).place(level, generator, new WorldgenRandom(seed), center);

  const monsterRoomCenter = new BlockPos(216, 192, 168);
  const monsterRoomPreview = new WorldgenRandom(seed);
  const roomWidth = monsterRoomPreview.nextInt(2) + 2;
  const roomDepth = monsterRoomPreview.nextInt(2) + 2;
  const minX = -roomWidth - 1;
  const maxX = roomWidth + 1;
  const minZ = -roomDepth - 1;
  const maxZ = roomDepth + 1;

  for (let offsetX = minX; offsetX <= maxX; offsetX++) {
    for (let offsetY = -1; offsetY <= 4; offsetY++) {
      for (let offsetZ = minZ; offsetZ <= maxZ; offsetZ++) {
        level.setBlock(monsterRoomCenter.offset(offsetX, offsetY, offsetZ), stoneState, 2);
      }
    }
  }

  level.setBlock(monsterRoomCenter.offset(minX, 0, 0), airState, 2);
  level.setBlock(monsterRoomCenter.offset(minX, 1, 0), airState, 2);

  Features.MONSTER_ROOM.configured(NoneFeatureConfiguration.INSTANCE).place(
    level,
    generator,
    new WorldgenRandom(seed),
    monsterRoomCenter,
  );

  for (let offsetX = minX; offsetX <= maxX; offsetX++) {
    for (let offsetZ = minZ; offsetZ <= maxZ; offsetZ++) {
      level.setBlock(monsterRoomCenter.offset(offsetX, 4, offsetZ), airState, 2);
    }
  }

  const buriedTreasureX = 120;
  const buriedTreasureZ = 168;
  for (let offsetX = -5; offsetX <= 5; offsetX++) {
    for (let offsetZ = -5; offsetZ <= 5; offsetZ++) {
      level.setBlock(new BlockPos(buriedTreasureX + offsetX, 79, buriedTreasureZ + offsetZ), stoneState, 2);
      for (let y = 80; y <= 84; y++) {
        level.setBlock(new BlockPos(buriedTreasureX + offsetX, y, buriedTreasureZ + offsetZ), sandState, 2);
      }
    }
  }

  const buriedTreasurePiece = new BuriedTreasurePiece(new BlockPos(buriedTreasureX, 90, buriedTreasureZ));
  buriedTreasurePiece.postProcess(
    level,
    null as never,
    generator,
    new WorldgenRandom(seed ^ 0x0b7313dn),
    new BoundingBox(buriedTreasureX - 8, 76, buriedTreasureZ - 8, buriedTreasureX + 8, 90, buriedTreasureZ + 8),
    new ChunkPos(Math.floor(buriedTreasureX / 16), Math.floor(buriedTreasureZ / 16)),
    new BlockPos(buriedTreasureX, 80, buriedTreasureZ),
  );

  const buriedTreasureBox = buriedTreasurePiece.getBoundingBox();
  const buriedTreasureChest = new BlockPos(buriedTreasureBox.minX(), buriedTreasureBox.minY(), buriedTreasureBox.minZ());
  const buriedTreasureGallery = new BoundingBox(
    buriedTreasureChest.getX() - 6,
    buriedTreasureChest.getY() - 2,
    buriedTreasureChest.getZ() - 8,
    buriedTreasureChest.getX() + 6,
    buriedTreasureChest.getY() + 5,
    buriedTreasureChest.getZ() + 4,
  );

  for (let y = buriedTreasureGallery.minY(); y <= buriedTreasureGallery.maxY(); y++) {
    for (let z = buriedTreasureGallery.minZ(); z <= buriedTreasureGallery.maxZ(); z++) {
      for (let x = buriedTreasureGallery.minX(); x <= buriedTreasureGallery.maxX(); x++) {
        if (x === buriedTreasureChest.getX() && y === buriedTreasureChest.getY() && z === buriedTreasureChest.getZ()) {
          continue;
        }

        const isShell =
          x === buriedTreasureGallery.minX() ||
          x === buriedTreasureGallery.maxX() ||
          y === buriedTreasureGallery.minY() ||
          y === buriedTreasureGallery.maxY() ||
          z === buriedTreasureGallery.minZ() ||
          z === buriedTreasureGallery.maxZ();
        level.setBlock(new BlockPos(x, y, z), isShell ? sandstoneState : airState, 2);
      }
    }
  }

  for (let y = buriedTreasureChest.getY(); y <= buriedTreasureChest.getY() + 3; y++) {
    for (let x = buriedTreasureChest.getX() - 2; x <= buriedTreasureChest.getX() + 2; x++) {
      level.setBlock(new BlockPos(x, y, buriedTreasureGallery.minZ()), airState, 2);
    }
  }

  const buriedTreasureClearance = new BoundingBox(
    buriedTreasureGallery.minX() - 6,
    buriedTreasureGallery.minY() - 2,
    buriedTreasureGallery.minZ() - 6,
    buriedTreasureGallery.maxX() + 6,
    buriedTreasureGallery.maxY() + 4,
    buriedTreasureGallery.maxZ() + 6,
  );
  for (let y = buriedTreasureClearance.minY(); y <= buriedTreasureClearance.maxY(); y++) {
    for (let z = buriedTreasureClearance.minZ(); z <= buriedTreasureClearance.maxZ(); z++) {
      for (let x = buriedTreasureClearance.minX(); x <= buriedTreasureClearance.maxX(); x++) {
        const pos = new BlockPos(x, y, z);
        if (buriedTreasureGallery.isInside(pos)) {
          continue;
        }
        level.setBlock(pos, airState, 2);
      }
    }
  }

  const fossilOrigin = new BlockPos(48, 0, 48);
  const boneBlock = getRequiredState("minecraft:bone_block").getBlock();
  const coalOreBlock = getRequiredState("minecraft:coal_ore").getBlock();
  const fossilFeature = Features.FOSSIL.configured(OVERWORLD_FOSSIL_CONFIGURATION);
  let fossilBlocks: { pos: BlockPos, state: BlockState }[] = [];

  for (let attempt = 0n; attempt < 256n; attempt++) {
    for (let y = 0; y <= 63; y++) {
      for (let z = 40; z <= 63; z++) {
        for (let x = 40; x <= 63; x++) {
          level.setBlock(new BlockPos(x, y, z), stoneState, 2);
        }
      }
    }

    const placed = fossilFeature.place(level, generator, new WorldgenRandom((seed ^ 0xf0551n) + attempt), fossilOrigin);
    if (!placed) {
      continue;
    }

    const candidateBlocks: { pos: BlockPos, state: BlockState }[] = [];
    let hasBone = false;
    let hasCoal = false;
    for (let y = level.getMinBuildHeight(); y < level.getMaxBuildHeight(); y++) {
      for (let z = 48; z <= 63; z++) {
        for (let x = 48; x <= 63; x++) {
          const pos = new BlockPos(x, y, z);
          const state = level.getBlockState(pos);
          if (state.is(boneBlock) || state.is(coalOreBlock)) {
            candidateBlocks.push({ pos, state });
            hasBone ||= state.is(boneBlock);
            hasCoal ||= state.is(coalOreBlock);
          }
        }
      }
    }

    if (hasBone && hasCoal) {
      fossilBlocks = candidateBlocks;
      break;
    }
  }

  const fossilBox = BoundingBox.encapsulatingPositions(fossilBlocks.map((block) => block.pos));
  if (fossilBox === undefined) {
    throw new Error("Could not place a deterministic fossil in the browser_smoke world");
  }

  const fossilWidth = fossilBox.getXSpan();
  const fossilHeight = fossilBox.getYSpan();
  const fossilDepth = fossilBox.getZSpan();
  const normalizedBlocks = fossilBlocks.map((block) => ({
    pos: new BlockPos(
      block.pos.getX() - fossilBox.minX(),
      block.pos.getY() - fossilBox.minY(),
      block.pos.getZ() - fossilBox.minZ(),
    ),
    state: block.state,
  }));
  const galleryShell = new BoundingBox(48, 28, 48, 94, 52, 94);
  const galleryCenterX = Math.floor((galleryShell.minX() + galleryShell.maxX()) / 2);
  const galleryCenterZ = Math.floor((galleryShell.minZ() + galleryShell.maxZ()) / 2);
  const centeredCopyX = galleryCenterX - Math.floor(fossilWidth / 2);
  const centeredCopyZ = galleryCenterZ - Math.floor(fossilDepth / 2);
  const wallMargin = 3;
  const floorY = galleryShell.minY() + 1;
  const displayOrigins = [
    new BlockPos(centeredCopyX, floorY, centeredCopyZ),
    new BlockPos(centeredCopyX, floorY, galleryShell.minZ() + wallMargin),
    new BlockPos(centeredCopyX, floorY, galleryShell.maxZ() - wallMargin - fossilDepth + 1),
    new BlockPos(galleryShell.minX() + wallMargin, floorY, galleryCenterZ - Math.floor(fossilDepth / 2)),
    new BlockPos(galleryShell.maxX() - wallMargin - fossilWidth + 1, floorY, galleryCenterZ - Math.floor(fossilDepth / 2)),
    new BlockPos(centeredCopyX, galleryShell.maxY() - wallMargin - fossilHeight + 1, centeredCopyZ),
  ];

  for (let y = galleryShell.minY(); y <= galleryShell.maxY(); y++) {
    for (let z = galleryShell.minZ(); z <= galleryShell.maxZ(); z++) {
      for (let x = galleryShell.minX(); x <= galleryShell.maxX(); x++) {
        const isShell =
          x === galleryShell.minX() ||
          x === galleryShell.maxX() ||
          y === galleryShell.minY() ||
          y === galleryShell.maxY() ||
          z === galleryShell.minZ() ||
          z === galleryShell.maxZ();
        level.setBlock(new BlockPos(x, y, z), isShell ? sandstoneState : airState, 2);
      }
    }
  }

  for (const origin of displayOrigins) {
    for (const fossilBlock of normalizedBlocks) {
      level.setBlock(
        origin.offset(fossilBlock.pos.getX(), fossilBlock.pos.getY(), fossilBlock.pos.getZ()),
        fossilBlock.state,
        2,
      );
    }
  }
}

export interface CreateGeneratedWorldHostOptions {
  readonly lightingMode?: WorldEngineLightingMode;
  readonly liquidSimulationMode?: WorldEngineLiquidSimulationMode;
  readonly chunkViewScheduling?: "synchronous" | "cooperative";
  readonly worldStorage?: WorldStorage;
  readonly lightingService?: LightingService;
}

export function createGeneratedWorldHostForRequest(
  request: OpenWorldRequest,
  options: CreateGeneratedWorldHostOptions = {},
): GeneratedWorldHost {
  const generatedBlocks = registerGeneratedRenderBlocks();
  const generator = createWorldGeneratorForPreset(request.preset, request.seed);

  return new GeneratedWorldHost({
    seed: request.seed,
    generator,
    airState: generatedBlocks.airState,
    blockStateById: generatedBlocks.blockStateById,
    blockStateIds: generatedBlocks.blockStateIds,
    lightingMode: options.lightingMode ?? request.config?.lightingMode,
    liquidSimulationMode: options.liquidSimulationMode ?? request.config?.liquidSimulationMode,
    chunkViewScheduling: options.chunkViewScheduling,
    worldStorage: options.worldStorage,
    lightingService: options.lightingService,
    mutateWorld:
      request.preset === "browser_smoke"
        ? (level) =>
            applySmokeWorldMutations(
              level,
              generatedBlocks.airState,
              generatedBlocks.blockStateById[ChunkBlockId.WATER]!,
              generatedBlocks.blockStateById[ChunkBlockId.STONE]!,
              generatedBlocks.blockStateById[ChunkBlockId.SAND]!,
              generatedBlocks.blockStateById[ChunkBlockId.SANDSTONE]!,
              generator,
              request.seed,
            )
        : undefined,
  });
}
