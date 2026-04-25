import { describe, expect, test } from "vitest";
import { createIntegratedServer } from "../../src/runtime/host/integrated-server";
import type { WorldHost } from "../../src/runtime/protocol/world-host";
import type {
  OpenWorldRequest,
  PollWorldUpdatesRequest,
  SetChunkViewRequest,
  SetPlayerInputRequest,
  WorldHostMessage,
} from "../../src/runtime/protocol/world-messages";

const OPEN_WORLD_REQUEST: OpenWorldRequest = {
  type: "open_world",
  seed: 12345n,
  preset: "default",
};

class RecordingWorldHost implements WorldHost {
  public closeCount = 0;

  public openWorld(_request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    return Promise.resolve([{
      type: "world_opened",
      minBuildHeight: 0,
      height: 256,
      saveMetadata: {
        saveId: "test-save",
        storageVersion: 1,
        seed: "12345",
        preset: "default",
        minBuildHeight: 0,
        height: 256,
        createdAtMs: 1,
        lastOpenedAtMs: 1,
      },
    }]);
  }

  public setChunkView(_request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    return Promise.resolve([]);
  }

  public setPlayerInput(_request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]> {
    return Promise.resolve([]);
  }

  public pollUpdates(_request: PollWorldUpdatesRequest): Promise<readonly WorldHostMessage[]> {
    return Promise.resolve([]);
  }

  public close(): void {
    this.closeCount++;
  }
}

describe("IntegratedServer facade", () => {
  test("tracks local server lifecycle around the shared host protocol", async () => {
    const host = new RecordingWorldHost();
    const server = createIntegratedServer(host);

    expect(server.getLifecycleState()).toBe("created");
    expect(server.isReady()).toBe(false);
    expect(server.isClosed()).toBe(false);

    await expect(server.openWorld(OPEN_WORLD_REQUEST)).resolves.toMatchObject([{ type: "world_opened" }]);
    expect(server.getLifecycleState()).toBe("running");
    expect(server.isReady()).toBe(true);

    server.pause();
    expect(server.getLifecycleState()).toBe("paused");
    expect(server.isPaused()).toBe(true);
    await expect(server.pollUpdates({ type: "poll_world_updates" })).resolves.toEqual([]);

    server.resume();
    expect(server.getLifecycleState()).toBe("running");
    expect(server.isPaused()).toBe(false);

    server.close();
    server.close();
    expect(server.getLifecycleState()).toBe("closed");
    expect(server.isClosed()).toBe(true);
    expect(host.closeCount).toBe(1);
    expect(() => server.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    })).toThrow("IntegratedServer.setChunkView() called after close()");
  });
});
