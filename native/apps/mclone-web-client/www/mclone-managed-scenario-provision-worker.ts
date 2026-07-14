import {
  openWorldDb,
  provisionIndexedDbManagedScenarioWorld,
  setIndexedDbCatalogPolicy,
} from "./mclone-web-world-catalog.js";

interface ManagedProvisionWorkerRequest {
  kind: "provision";
  bindgenJsUrl: string;
  bindgenWasmUrl: string;
  operationToken: string;
  scenarioId: string;
  role: string;
}

const scope = self as DedicatedWorkerGlobalScope;

scope.onmessage = (event: MessageEvent<ManagedProvisionWorkerRequest>): void => {
  void run(event.data);
};

async function run(request: ManagedProvisionWorkerRequest): Promise<void> {
  let db: IDBDatabase | null = null;
  try {
    if (request.kind !== "provision") {
      throw new Error(`unsupported managed-content Worker request ${String(request.kind)}`);
    }
    const wasm = await import(request.bindgenJsUrl) as Record<string, any>;
    if (typeof wasm.default !== "function") {
      throw new Error("managed-content Worker wasm module has no initializer");
    }
    await wasm.default(request.bindgenWasmUrl);
    setIndexedDbCatalogPolicy(wasm as any);
    db = await openWorldDb();
    const result = await provisionIndexedDbManagedScenarioWorld(
      db,
      request.operationToken,
      request.scenarioId,
      request.role,
    );
    scope.postMessage({ ok: true, result });
  } catch (error) {
    scope.postMessage({
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
  } finally {
    db?.close();
    scope.close();
  }
}
