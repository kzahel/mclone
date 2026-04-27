import { describe, expect, test } from "vitest";
import { BotRuntime, IdleBotController, WanderBotController, createBotClientRuntime } from "../../src/runtime/bot";
import { parseBotClientConfig } from "../../src/runtime/node/bot-client";
import type { WorldTransport } from "../../src/runtime/transport/local-world-transport";
import type {
  ClientPlayerState,
  ClientSessionState,
  OpenWorldRequest,
  PollWorldUpdatesRequest,
  SetChunkViewRequest,
  SetPlayerInputRequest,
  WorldHostMessage,
  WorldOpenedMessage,
} from "../../src/runtime/protocol/world-messages";

const OPEN_WORLD_REQUEST: OpenWorldRequest = {
  type: "open_world",
  seed: 12345n,
  preset: "default",
  playerProfile: {
    name: "TestBot",
  },
};

class RecordingBotTransport implements WorldTransport {
  public readonly openWorldRequests: OpenWorldRequest[] = [];
  public readonly chunkViewRequests: SetChunkViewRequest[] = [];
  public readonly playerInputRequests: SetPlayerInputRequest[] = [];
  public readonly pollUpdateRequests: PollWorldUpdatesRequest[] = [];
  public closeCount = 0;
  private sessionState: ClientSessionState | undefined;
  private playerState: ClientPlayerState | undefined;

  public openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    this.openWorldRequests.push(request);
    this.sessionState = {
      sessionId: "session-1",
      playerId: "player-1",
      playerProfile: request.playerProfile ?? { name: "Player" },
      saveId: "test-save",
      resumed: false,
      revision: 0,
    };
    this.playerState = {
      playerId: "player-1",
      position: {
        x: 8.5,
        y: 168.0,
        z: 8.5,
      },
      rotation: {
        yaw: 0.0,
        pitch: 0.0,
      },
      acknowledgedInputSequence: 0,
      tick: 0,
      revision: 0,
    };
    const opened: WorldOpenedMessage = {
      type: "world_opened",
      minBuildHeight: 0,
      height: 256,
      saveMetadata: {
        saveId: "test-save",
        storageVersion: 1,
        seed: request.seed.toString(),
        preset: request.preset,
        minBuildHeight: 0,
        height: 256,
        createdAtMs: 1,
        lastOpenedAtMs: 1,
      },
    };
    return Promise.resolve([
      opened,
      {
        type: "session_state",
        state: this.sessionState,
      },
      {
        type: "player_state",
        state: this.playerState,
      },
    ]);
  }

  public setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    this.chunkViewRequests.push(request);
    this.sessionState = {
      ...this.requireSessionState(),
      revision: this.requireSessionState().revision + 1,
      chunkView: {
        centerChunkX: request.centerChunkX,
        centerChunkZ: request.centerChunkZ,
        radius: request.radius,
      },
    };
    return Promise.resolve([{
      type: "session_state",
      state: this.sessionState,
    }]);
  }

  public setPlayerInput(request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]> {
    this.playerInputRequests.push(request);
    const playerState = this.requirePlayerState();
    this.playerState = {
      ...playerState,
      rotation: {
        yaw: request.input.yaw,
        pitch: request.input.pitch,
      },
      acknowledgedInputSequence: request.input.sequence,
      tick: playerState.tick + 1,
      revision: playerState.revision + 1,
    };
    return Promise.resolve([{
      type: "player_state",
      state: this.playerState,
    }]);
  }

  public pollUpdates(request: PollWorldUpdatesRequest): Promise<readonly WorldHostMessage[]> {
    this.pollUpdateRequests.push(request);
    return Promise.resolve([]);
  }

  public close(): void {
    this.closeCount++;
  }

  private requireSessionState(): ClientSessionState {
    if (this.sessionState === undefined) {
      throw new Error("missing session state");
    }
    return this.sessionState;
  }

  private requirePlayerState(): ClientPlayerState {
    if (this.playerState === undefined) {
      throw new Error("missing player state");
    }
    return this.playerState;
  }
}

describe("BotRuntime", () => {
  test("opens through ClientRuntime, tracks chunk interest, and sends sequenced idle commands", async () => {
    let nowMs = 0;
    const transport = new RecordingBotTransport();
    const bot = new BotRuntime({
      clientRuntime: createBotClientRuntime({
        transport,
        seed: OPEN_WORLD_REQUEST.seed,
      }),
      openWorldRequest: OPEN_WORLD_REQUEST,
      viewRadius: 3,
      controller: new IdleBotController(),
      nowMs: () => nowMs,
    });

    const opened = await bot.open();
    expect(opened.saveMetadata.saveId).toBe("test-save");
    expect(transport.openWorldRequests).toHaveLength(1);
    expect(transport.openWorldRequests[0]?.playerProfile?.name).toBe("TestBot");
    expect(transport.chunkViewRequests).toEqual([{
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 3,
    }]);

    nowMs += 50;
    const firstTick = await bot.tick(50);
    nowMs += 50;
    const secondTick = await bot.tick(50);

    expect(firstTick.commandCount).toBe(1);
    expect(firstTick.chunkInterestChanged).toBe(false);
    expect(secondTick.commandCount).toBe(1);
    expect(transport.playerInputRequests.map((request) => request.input.sequence)).toEqual([1, 2]);
    expect(transport.playerInputRequests.every((request) => request.input.moveX === 0 && request.input.moveZ === 0)).toBe(true);
    expect(transport.pollUpdateRequests).toHaveLength(2);
    expect(bot.getStatus().nextInputSequence).toBe(3);

    await bot.close();
    await bot.close();
    expect(transport.closeCount).toBe(1);
  });

  test("wander controller produces forward movement and changing yaw", async () => {
    let nowMs = 0;
    const transport = new RecordingBotTransport();
    const bot = new BotRuntime({
      clientRuntime: createBotClientRuntime({
        transport,
        seed: OPEN_WORLD_REQUEST.seed,
      }),
      openWorldRequest: OPEN_WORLD_REQUEST,
      controller: new WanderBotController({ yawDegreesPerSecond: 60.0 }),
      nowMs: () => nowMs,
    });

    await bot.open();
    nowMs += 50;
    await bot.tick(50);
    nowMs += 50;
    await bot.tick(50);

    expect(transport.playerInputRequests).toHaveLength(2);
    expect(transport.playerInputRequests[0]?.input.moveZ).toBe(1);
    expect(transport.playerInputRequests[1]?.input.yaw).toBeGreaterThan(transport.playerInputRequests[0]!.input.yaw);

    await bot.close();
  });
});

describe("bot client CLI config", () => {
  test("parses explicit CLI options", () => {
    expect(parseBotClientConfig([
      "--url",
      "ws://localhost:4173/api/world/socket",
      "--name",
      "ViewBot",
      "--seed",
      "99",
      "--preset",
      "browser_smoke",
      "--radius",
      "4",
      "--tick-rate",
      "10",
      "--controller",
      "idle",
      "--max-ticks",
      "12",
    ])).toEqual({
      url: "ws://localhost:4173/api/world/socket",
      name: "ViewBot",
      seed: "99",
      preset: "browser_smoke",
      radius: 4,
      tickRate: 10,
      controller: "idle",
      maxTicks: 12,
    });
  });

  test("rejects invalid options", () => {
    expect(() => parseBotClientConfig(["--seed", "nope"])).toThrow("Expected seed");
    expect(() => parseBotClientConfig(["--controller", "good-view"])).toThrow("Unsupported bot controller");
  });
});
