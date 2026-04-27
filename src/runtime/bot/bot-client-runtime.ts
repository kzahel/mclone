import { WorldClientRuntimeFacade, type ClientRuntime } from "../client/client-runtime";
import type { WorldProgressMessage, WorldOpenedMessage } from "../protocol/world-messages";
import { TransportWorldClient, type WorldTransport } from "../transport/local-world-transport";
import { ClientChunkCache } from "../../world/level/client-chunk-cache";
import { createBlockStateResolver } from "../../world/level/chunk-snapshot";
import { registerGeneratedRenderBlocks } from "../../world/level/generated-render-blocks";
import { OverworldBiomeSource } from "../../worldgen/biome/overworld-biome-source";

export interface BotClientRuntimeOptions {
  readonly transport: WorldTransport;
  readonly seed: bigint;
  readonly pollUpdateMaxMessages?: number;
  readonly worldProgressSink?: (message: WorldProgressMessage) => void;
}

export function createBotClientRuntime(options: BotClientRuntimeOptions): ClientRuntime {
  const generatedBlocks = registerGeneratedRenderBlocks();
  const biomeSource = new OverworldBiomeSource(options.seed);
  const blockStateResolver = createBlockStateResolver(generatedBlocks.airState);
  const levelFactory = (worldOpened: WorldOpenedMessage) => new ClientChunkCache({
    airState: generatedBlocks.airState,
    minBuildHeight: worldOpened.minBuildHeight,
    height: worldOpened.height,
    biomeSource,
    biomeZoomSeed: options.seed,
    blockStateResolver,
    blockStateIds: generatedBlocks.blockStateIds,
  });

  return new WorldClientRuntimeFacade(new TransportWorldClient(options.transport, levelFactory, {
    pollUpdateMaxMessages: options.pollUpdateMaxMessages,
    worldProgressSink: options.worldProgressSink,
  }));
}
