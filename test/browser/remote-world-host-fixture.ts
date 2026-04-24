import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { expect, test as base } from "@playwright/test";
import type { Page } from "@playwright/test";
import { GeneratedWorldHttpServer } from "../../src/runtime/node/generated-world-http-server";

interface RemoteWorldHostFixtures {
  readonly remoteWorldHostUrl: string;
}

export const test = base.extend<RemoteWorldHostFixtures>({
  remoteWorldHostUrl: async ({}, use) => {
    const saveRoot = await mkdtemp(path.join(tmpdir(), "mclone-browser-remote-"));
    const server = new GeneratedWorldHttpServer({
      host: "127.0.0.1",
      port: 0,
      saveRoot,
    });

    await server.start();
    try {
      await use(server.getBaseUrl());
    } finally {
      await server.stop();
      await rm(saveRoot, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
    }
  },
});

export { expect };
export type { Page };
