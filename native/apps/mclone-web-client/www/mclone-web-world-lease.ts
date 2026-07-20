export interface HeldWorldWriterLease {
  release(): Promise<void>;
}

export async function acquireWorldWriterLease(
  name: string,
): Promise<HeldWorldWriterLease> {
  const manager = (globalThis.navigator as unknown as {
    locks?: {
      request<T>(
        name: string,
        options: { mode: "exclusive"; ifAvailable: true },
        callback: (lock: unknown | null) => Promise<T>,
      ): Promise<T>;
    };
  }).locks;
  if (!manager) {
    throw new Error("persistent browser worlds require the Worker-visible Web Locks API");
  }

  let resolveAdmission: ((acquired: boolean) => void) | null = null;
  const admission = new Promise<boolean>((resolve) => {
    resolveAdmission = resolve;
  });
  let resolveRelease: (() => void) | null = null;
  const releaseSignal = new Promise<void>((resolve) => {
    resolveRelease = resolve;
  });
  const held = manager.request(
    name,
    { mode: "exclusive", ifAvailable: true },
    async (lock) => {
      resolveAdmission?.(lock !== null);
      if (lock === null) return;
      await releaseSignal;
    },
  );
  if (!await admission) {
    await held;
    throw new Error("this world is already open in another browser window");
  }
  let released = false;
  return {
    async release(): Promise<void> {
      if (released) return;
      released = true;
      resolveRelease?.();
      await held;
    },
  };
}
