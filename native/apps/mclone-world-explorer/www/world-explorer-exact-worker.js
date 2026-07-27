import initModule, {
  WorldExplorerExactWorkerActor,
} from "./pkg/mclone_world_explorer.js?v=__MCLONE_WORLD_EXPLORER_ASSET_VERSION__";

let modulePromise;
let actor;

self.onmessage = (event) => {
  void forward(event.data).catch((error) => {
    console.error("World Explorer exact Worker bootstrap failed:", error);
    self.postMessage({
      kind: "exact-error",
      message: error instanceof Error ? error.message : String(error),
    });
  });
};

async function forward(message) {
  modulePromise ??= initModule(new URL(
    "./pkg/mclone_world_explorer_bg.wasm?v=__MCLONE_WORLD_EXPLORER_ASSET_VERSION__",
    import.meta.url,
  ));
  await modulePromise;
  actor ??= new WorldExplorerExactWorkerActor();
  const dispatch = actor.handleMessage(message);
  try {
    const response = dispatch.message;
    const transfer = response.bytes instanceof Uint8Array
      ? [response.bytes.buffer]
      : [];
    self.postMessage(response, transfer);
  } finally {
    dispatch.free();
  }
}
