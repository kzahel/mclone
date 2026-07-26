import initModule, {
  WorldExplorerWorkerActor,
} from "./pkg/mclone_world_explorer.js";

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
  modulePromise ??= initModule();
  await modulePromise;
  actor ??= new WorldExplorerWorkerActor();
  const dispatch = actor.handleMessage(message);
  try {
    self.postMessage(dispatch.message);
  } finally {
    dispatch.free();
  }
}
