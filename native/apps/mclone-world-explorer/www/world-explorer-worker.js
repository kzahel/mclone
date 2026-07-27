import initModule, {
  WorldExplorerWorkerActor,
} from "./pkg/mclone_world_explorer.js?v=__MCLONE_WORLD_EXPLORER_ASSET_VERSION__";

let modulePromise;
let actor;

self.onmessage = (event) => {
  void forward(event.data).catch((error) => {
    console.error("World Explorer Worker bootstrap failed:", error);
    self.postMessage({
      error: error instanceof Error ? error.message : String(error),
    });
  });
};

async function forward(message) {
  modulePromise ??= initModule(new URL(
    "./pkg/mclone_world_explorer_bg.wasm?v=__MCLONE_WORLD_EXPLORER_ASSET_VERSION__",
    import.meta.url,
  ));
  await modulePromise;
  actor ??= new WorldExplorerWorkerActor();
  const dispatch = actor.handleMessage(message);
  try {
    self.postMessage(dispatch.message);
  } finally {
    dispatch.free();
  }
}
