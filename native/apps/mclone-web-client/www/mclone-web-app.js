const DEPLOY_ASSET_VERSION = normalizedDeployAssetVersion();
const BINDGEN_JS_URL = versionedUrl("./pkg/mclone_web_client.js");
const BINDGEN_WASM_URL = versionedUrl("./pkg/mclone_web_client_bg.wasm");
const RENDER_COMPILER_WORKER_URL = versionedUrl("./mclone-render-compiler-worker.js");
const SERVER_WORKER_URL = versionedUrl("./mclone-integrated-server-worker.js");
const SERVER_JOB_WORKER_URL = versionedUrl("./mclone-server-job-worker.js");
const ASSET_PACK_URL = versionedUrl("/reference/minecraft-1.17.1/extracted.zip");

const RADIUS_CHUNKS = 1;
const MAX_FRAME_DT_SECONDS = 0.05;
const INPUT_KEY_NAMES = ["forward", "backward", "left", "right", "jump", "descend", "shift", "sprint"];
const TOUCH_JOYSTICK_MAX_DISTANCE = 50;
const TOUCH_JOYSTICK_DEAD_ZONE = 10;
const TOUCH_AXIS_THRESHOLD = TOUCH_JOYSTICK_DEAD_ZONE / TOUCH_JOYSTICK_MAX_DISTANCE;

const runtime = {
  ready: false,
  state: {
    ok: false,
    ready: false,
    failed: false,
    centerX: 0,
    centerZ: 0,
    loadedCenterX: null,
    loadedCenterZ: null,
    radiusChunks: RADIUS_CHUNKS,
    cameraX: 0,
    cameraY: 0,
    cameraZ: 0,
    cameraYawRadians: 0,
    cameraPitchRadians: 0,
    cameraSpeedBlocksPerSecond: 0,
    movementMode: "WALK",
    onGround: false,
    horizontalCollision: false,
    verticalCollision: false,
    selectedHotbarSlot: 0,
    currentTarget: null,
    interactionCount: 0,
    interactionStatus: "idle",
    lastInteraction: null,
    width: 0,
    height: 0,
    dayTime: 0,
    timeOfDay: 0,
    skyRendered: false,
    actorCount: 0,
    drawnActorCount: 0,
    loadedChunkCount: 0,
    residentSectionCount: 0,
    pendingCompileJobCount: 0,
    compileInFlight: false,
    compileQueued: false,
    compileTargetX: null,
    compileTargetZ: null,
    queuedCompileTargetX: null,
    queuedCompileTargetZ: null,
    activeCompileTiming: null,
    lastCompileTiming: null,
    compileTimings: [],
    compileTimingCount: 0,
    renderCount: 0,
    frameCount: 0,
    lastFrameGapMs: 0,
    maxFrameGapMs: 0,
    pointerLockSupported: false,
    pointerLockAttempted: false,
    pointerLocked: false,
    pointerLockFallback: false,
    tickFrameBusy: false,
    tickPhase: "idle",
    runnerKind: "unknown",
    runnerCommandQueueDepth: 0,
    runnerUpdateQueueDepth: 0,
    runnerPendingJobs: 0,
    runnerPendingPublications: 0,
    worldgenMailboxKind: "unknown",
    lightStatusMailboxKind: "unknown",
    worldgenMailboxPendingJobs: 0,
    lightStatusMailboxPendingStatuses: 0,
    runnerFrameMetrics: null,
    worldgenJobFrameMetrics: null,
    lightStatusJobFrameMetrics: null,
    hudOpen: true,
    touchControlsVisible: false,
    touchJoystickActive: false,
    touchMovementLeftImpulse: 0,
    touchMovementForwardImpulse: 0,
    touchLookActive: false,
    touchButtonActiveCount: 0,
    status: "booting",
  },
};

globalThis.__mcloneWebApp = runtime;

async function boot() {
  const app = new WebChunkApp();
  runtime.queueMouseDelta = (dx, dy) => app.queueMouseDelta(dx, dy);
  runtime.setInputKey = (name, down) => app.setInputKey(name, down);
  runtime.adjustCameraSpeed = (amount) => app.adjustCameraSpeed(amount);
  runtime.previewBlockTarget = () => runtime.state.currentTarget;
  runtime.touchControlState = () => app.touchControls?.snapshot() ?? null;
  runtime.setHudOpen = (open) => setHudOpen(Boolean(open));
  try {
    await app.init();
    runtime.ready = true;
    runtime.state.ready = true;
    runtime.state.ok = true;
    runtime.state.failed = false;
    runtime.state.status = "ready";
    app.start();
    return snapshotState();
  } catch (error) {
    runtime.ready = false;
    runtime.state.ok = false;
    runtime.state.failed = true;
    runtime.state.status = stringifyError(error);
    updateDom();
    return snapshotState();
  }
}

class WebChunkApp {
  constructor() {
    this.canvas = document.getElementById("mclone-canvas");
    this.status = document.getElementById("status");
    this.hud = document.getElementById("runtime-hud");
    this.hudToggle = document.getElementById("hud-toggle");
    this.session = null;
    this.compiler = null;
    this.keys = defaultInputKeys();
    this.touchKeys = defaultInputKeys();
    this.touchMovementImpulse = defaultMovementImpulse();
    this.touchControls = null;
    this.mouseDeltaX = 0;
    this.mouseDeltaY = 0;
    this.pendingCompile = false;
    this.currentCompilePromise = null;
    this.compileSequence = 0;
    this.activeCompileTiming = null;
    this.compileInFlightTarget = null;
    this.renderFrameBusy = false;
    this.hasRendered = false;
    this.loadedCenter = null;
    this.pointerDragging = false;
    this.pointerDown = null;
    this.animationFrame = 0;
    this.lastFrameTime = 0;
    this.tickFrameBusy = false;
    this.sessionBusy = false;
  }

  async init() {
    if (!(this.canvas instanceof HTMLCanvasElement)) {
      throw new Error("missing canvas#mclone-canvas");
    }
    runtime.state.pointerLockSupported = typeof this.canvas.requestPointerLock === "function";
    this.canvas.focus();

    runtime.state.status = "loading wasm";
    updateDom();
    const module = await import(BINDGEN_JS_URL.href);
    await module.default(BINDGEN_WASM_URL.href);
    for (const name of ["mclone_web_create_worker_chunk_render_session"]) {
      if (typeof module[name] !== "function") {
        throw new Error(`missing ${name} export`);
      }
    }

    runtime.state.status = "loading assets";
    updateDom();
    const assetPack = await fetchAssetPack();
    runtime.state.status = "initializing webgpu";
    updateDom();
    this.session = await module.mclone_web_create_worker_chunk_render_session(
      this.canvas,
      assetPack,
      SERVER_WORKER_URL.href,
      SERVER_JOB_WORKER_URL.href,
      BINDGEN_JS_URL.href,
      BINDGEN_WASM_URL.href,
    );
    for (const name of [
      "advanceCameraFrame",
      "beginCameraRenderCompileRequest",
      "requestCameraRenderCompile",
      "takeQueuedCameraRenderCompile",
      "requestCameraChunkView",
      "tryBeginCameraRenderCompileRequest",
      "finishCameraRenderCompileRequest",
      "renderCameraFrame",
      "cameraFrameState",
      "resizeCanvas",
      "toggleMovementMode",
      "selectHotbarSlot",
      "previewBlockTarget",
      "interactBlock",
    ]) {
      if (typeof this.session[name] !== "function") {
        throw new Error(`missing WebChunkRenderSession.${name} export`);
      }
    }
    this.compiler = new RenderSectionWorkerCompiler(assetPack);
    bindHud(this);
    bindInput(this);
    this.touchControls = new TouchControls(this);
    this.syncCanvasSize();
    this.applyCameraState(this.session.cameraFrameState());
    this.applyTargetState(this.session.previewBlockTarget());

    runtime.state.status = "rendering";
    updateDom();
    await this.compileCameraView({ trigger: "initial", waitForCompletion: true });
  }

  start() {
    this.lastFrameTime = performance.now();
    const frame = (now) => {
      if (!this.tickFrameBusy) {
        this.tickFrameBusy = true;
        runtime.state.tickFrameBusy = true;
        void this.tickFrame(now).finally(() => {
          this.tickFrameBusy = false;
          runtime.state.tickFrameBusy = false;
          runtime.state.tickPhase = "idle";
        });
      }
      this.animationFrame = requestAnimationFrame(frame);
    };
    this.animationFrame = requestAnimationFrame(frame);
  }

  async tickFrame(now) {
    if (!this.session) {
      return;
    }
    const frameGapMs = Number.isFinite(now - this.lastFrameTime)
      ? Math.max(0, now - this.lastFrameTime)
      : 0;
    this.recordFrameGap(frameGapMs);
    const rawDt = frameGapMs / 1000;
    this.lastFrameTime = now;
    const dtSeconds = Number.isFinite(rawDt)
      ? Math.max(0, Math.min(rawDt, MAX_FRAME_DT_SECONDS))
      : 0;
    const mouseDeltaX = this.mouseDeltaX;
    const mouseDeltaY = this.mouseDeltaY;
    this.mouseDeltaX = 0;
    this.mouseDeltaY = 0;
    this.syncCanvasSize();
    const keys = this.currentInputKeys();
    const movementImpulse = this.currentMovementImpulse();

    runtime.state.frameCount += 1;
    runtime.state.tickPhase = "advance";
    const camera = await this.withSessionAsync(() => this.session.advanceCameraFrame(
      dtSeconds,
      mouseDeltaX,
      mouseDeltaY,
      keys.forward,
      keys.backward,
      keys.left,
      keys.right,
      keys.jump,
      keys.descend,
      keys.shift,
      keys.sprint,
      movementImpulse.active,
      movementImpulse.left,
      movementImpulse.forward,
    ));
    runtime.state.tickPhase = "post-advance";
    this.applyCameraState(camera);
    this.applyTargetState(this.session.previewBlockTarget());

    if (this.hasRendered && this.loadedCenter) {
      if (
        camera.centerX !== this.loadedCenter.centerX
        || camera.centerZ !== this.loadedCenter.centerZ
      ) {
        runtime.state.tickPhase = "compile-requested";
        this.requestCameraViewCompile("movement");
      }
    }

    if (this.hasRendered && !this.renderFrameBusy) {
      runtime.state.tickPhase = "render";
      this.renderCameraFrame();
    } else {
      updateDom();
    }
    this.startQueuedCompileIfNeeded();
  }

  requestCameraViewCompile(trigger = "movement", options = {}) {
    if (!this.session || !this.compiler) {
      return null;
    }
    const force = Boolean(options.force);
    const decision = this.session.requestCameraRenderCompile(
      trigger,
      force,
      this.pendingCompile,
    );
    this.applyCompileQueueDecision(decision);
    if (!decision?.startNow) {
      updateDom();
      return this.currentCompilePromise;
    }
    void this.compileCameraView({
      trigger: decision.trigger ?? trigger,
      force: Boolean(decision.force),
      targetCenter: decisionCenter(decision),
      waitForCompletion: false,
    });
    return this.currentCompilePromise;
  }

  async compileCameraView(options = {}) {
    const trigger = options.trigger ?? "manual";
    const force = Boolean(options.force);
    const waitForCompletion = options.waitForCompletion !== false;
    if (!this.session || !this.compiler) {
      return null;
    }
    const targetCenter = options.targetCenter ?? this.currentCameraCenter();
    if (!force && this.hasRendered && this.loadedCenter && centersEqual(targetCenter, this.loadedCenter)) {
      return null;
    }
    if (this.pendingCompile) {
      const pending = this.requestCameraViewCompile(trigger, { force });
      return waitForCompletion ? await pending : pending;
    }

    const sequence = this.compileSequence + 1;
    this.compileSequence = sequence;
    const timing = createCompileTiming({
      sequence,
      trigger,
      targetCenter,
      loadedCenterBefore: this.loadedCenter,
      frameCountBefore: runtime.state.frameCount,
      renderCountBefore: runtime.state.renderCount,
    });
    this.pendingCompile = true;
    this.activeCompileTiming = timing;
    this.compileInFlightTarget = {
      centerX: timing.targetCenterX,
      centerZ: timing.targetCenterZ,
    };
    runtime.state.compileInFlight = true;
    runtime.state.compileQueued = false;
    runtime.state.compileTargetX = timing.targetCenterX;
    runtime.state.compileTargetZ = timing.targetCenterZ;
    runtime.state.queuedCompileTargetX = null;
    runtime.state.queuedCompileTargetZ = null;
    runtime.state.status = this.hasRendered ? "ready" : "compiling";
    runtime.state.pendingCompileJobCount = this.session.pendingChunkRenderCompileJobCount();
    publishActiveCompileTiming(timing);
    updateDom();

    this.currentCompilePromise = (async () => {
      try {
        const beginStart = performance.now();
        const request = this.hasRendered
          ? await this.beginDeferredCameraRenderCompileRequest(timing, beginStart)
          : await this.withSessionAsync(() => (
              this.session.beginCameraRenderCompileRequest(RADIUS_CHUNKS)
            ));
        const beginEnd = performance.now();
        timing.beginRequestMs = beginEnd - beginStart;
        if (!request?.ok) {
          throw new Error(request?.reason ?? "failed to begin camera render compile request");
        }
        updateCompileTimingFromRequest(timing, request);
        this.compileInFlightTarget = {
          centerX: timing.targetCenterX,
          centerZ: timing.targetCenterZ,
        };
        runtime.state.compileTargetX = timing.targetCenterX;
        runtime.state.compileTargetZ = timing.targetCenterZ;
        runtime.state.pendingCompileJobCount = request.pendingCompileJobCount ?? 1;
        publishActiveCompileTiming(timing);
        updateDom();

        const workerStart = performance.now();
        const compiled = await this.compiler.compile(request);
        const workerEnd = performance.now();
        timing.workerRoundTripMs = workerEnd - workerStart;
        timing.packedByteLength = compiled.report?.packedByteLength ?? compiled.packed?.byteLength ?? 0;
        updateCompileTimingFromWorker(timing, compiled.report);
        publishActiveCompileTiming(timing);
        if (!compiled.report?.ok) {
          throw new Error(compiled.report?.reason ?? "render compiler worker failed");
        }

        const finishStart = performance.now();
        const report = await this.withSessionAsync(() => (
          this.session.finishCameraRenderCompileRequest(
            request.requestId,
            compiled.packed,
          )
        ));
        const finishEnd = performance.now();
        timing.decodeFinishApplyMs = finishEnd - finishStart;
        if (this.lastFrameTime > 0) {
          timing.maxFrameGapMs = Math.max(timing.maxFrameGapMs, performance.now() - this.lastFrameTime);
        }
        timing.maxFrameGapMs = Math.max(timing.maxFrameGapMs, timing.decodeFinishApplyMs);
        if (!report?.ok) {
          throw new Error(report?.reason ?? "failed to finish camera render compile request");
        }
        updateCompileTimingFromReport(timing, report);
        this.hasRendered = true;
        this.loadedCenter = { centerX: report.centerX, centerZ: report.centerZ };
        runtime.state.loadedCenterX = report.centerX;
        runtime.state.loadedCenterZ = report.centerZ;
        runtime.state.lastCompileReport = report;
        this.applyReport(report);
        finishCompileTiming(timing, "accepted");
        recordCompileTiming(timing);
        return report;
      } catch (error) {
        finishCompileTiming(timing, "failed", stringifyError(error));
        recordCompileTiming(timing);
        runtime.state.ok = false;
        runtime.state.status = stringifyError(error);
        throw error;
      } finally {
        this.pendingCompile = false;
        this.currentCompilePromise = null;
        this.activeCompileTiming = null;
        this.compileInFlightTarget = null;
        runtime.state.compileInFlight = false;
        runtime.state.activeCompileTiming = null;
        runtime.state.compileTargetX = null;
        runtime.state.compileTargetZ = null;
        runtime.state.pendingCompileJobCount = this.session?.pendingChunkRenderCompileJobCount?.() ?? 0;
        updateDom();
        this.startQueuedCompileIfNeeded();
      }
    })();

    if (!waitForCompletion) {
      this.currentCompilePromise.catch((error) => console.error(error));
      return this.currentCompilePromise;
    }
    return await this.currentCompilePromise;
  }

  renderCameraFrame() {
    if (!this.session || !this.hasRendered) {
      return;
    }
    this.renderFrameBusy = true;
    try {
      const report = this.session.renderCameraFrame(RADIUS_CHUNKS);
      if (!report?.ok) {
        throw new Error(report?.reason ?? "failed to render camera frame");
      }
      this.applyReport(report);
    } catch (error) {
      runtime.state.ok = false;
      runtime.state.status = stringifyError(error);
      console.error(error);
    } finally {
      this.renderFrameBusy = false;
      updateDom();
    }
  }

  async beginDeferredCameraRenderCompileRequest(timing, beginStart) {
    const viewRequest = await this.withSessionAsync(() => (
      this.session.requestCameraChunkView(RADIUS_CHUNKS)
    ));
    if (!viewRequest?.ok) {
      throw new Error(viewRequest?.reason ?? "failed to request camera chunk view");
    }
    const viewCenterX = Number(viewRequest.centerX);
    const viewCenterZ = Number(viewRequest.centerZ);
    if (Number.isFinite(viewCenterX)) {
      timing.targetCenterX = viewCenterX;
    }
    if (Number.isFinite(viewCenterZ)) {
      timing.targetCenterZ = viewCenterZ;
    }
    updateCompileTimingFromWait(timing, viewRequest, beginStart);
    publishActiveCompileTiming(timing);
    updateDom();

    const deadline = performance.now() + 20_000;
    let lastProbe = viewRequest;
    while (performance.now() < deadline) {
      await nextAnimationFrame();
      const probe = await this.withSessionAsync(() => (
        this.session.tryBeginCameraRenderCompileRequest(
          RADIUS_CHUNKS,
          timing.targetCenterX,
          timing.targetCenterZ,
        )
      ));
      if (!probe?.ok) {
        throw new Error(probe?.reason ?? "failed to poll camera render compile request");
      }
      lastProbe = probe;
      runtime.state.pendingCompileJobCount = probe.pendingCompileJobCount ?? 0;
      updateCompileTimingFromWait(timing, probe, beginStart);
      publishActiveCompileTiming(timing);
      updateDom();
      if (!probe.waiting) {
        return probe;
      }
    }
    throw new Error(
      `timed out waiting for camera chunk view ${timing.targetCenterX},${timing.targetCenterZ}; last=${JSON.stringify(lastProbe)}`,
    );
  }

  applyCameraState(camera) {
    if (!camera?.ok) {
      return;
    }
    runtime.state.centerX = Number(camera.centerX) || 0;
    runtime.state.centerZ = Number(camera.centerZ) || 0;
    runtime.state.cameraX = Number(camera.cameraX) || 0;
    runtime.state.cameraY = Number(camera.cameraY) || 0;
    runtime.state.cameraZ = Number(camera.cameraZ) || 0;
    runtime.state.cameraYawRadians = Number(camera.cameraYawRadians) || 0;
    runtime.state.cameraPitchRadians = Number(camera.cameraPitchRadians) || 0;
    runtime.state.cameraSpeedBlocksPerSecond = Number(camera.cameraSpeedBlocksPerSecond) || 0;
    runtime.state.movementMode = String(camera.movementMode || runtime.state.movementMode || "WALK");
    runtime.state.onGround = Boolean(camera.onGround);
    runtime.state.horizontalCollision = Boolean(camera.horizontalCollision);
    runtime.state.verticalCollision = Boolean(camera.verticalCollision);
    applyHotbarState(camera);
  }

  applyReport(report) {
    this.applyCameraState(report);
    runtime.state.ok = true;
    runtime.state.radiusChunks = report.radiusChunks;
    runtime.state.width = report.width;
    runtime.state.height = report.height;
    runtime.state.dayTime = report.dayTime;
    runtime.state.timeOfDay = report.timeOfDay;
    runtime.state.skyRendered = Boolean(report.skyRendered);
    runtime.state.actorCount = report.actorCount;
    runtime.state.drawnActorCount = report.drawnActorCount;
    runtime.state.loadedChunkCount = report.loadedChunkCount;
    runtime.state.residentSectionCount = report.residentSectionCount;
    runtime.state.pendingCompileJobCount = report.pendingCompileJobCount;
    runtime.state.runnerKind = report.runnerKind;
    runtime.state.runnerCommandQueueDepth = report.runnerCommandQueueDepth;
    runtime.state.runnerUpdateQueueDepth = report.runnerUpdateQueueDepth;
    runtime.state.runnerPendingJobs = report.runnerPendingJobs;
    runtime.state.runnerPendingPublications = report.runnerPendingPublications;
    runtime.state.worldgenMailboxKind = report.worldgenMailboxKind;
    runtime.state.lightStatusMailboxKind = report.lightStatusMailboxKind;
    runtime.state.worldgenMailboxPendingJobs = report.worldgenMailboxPendingJobs;
    runtime.state.lightStatusMailboxPendingStatuses = report.lightStatusMailboxPendingStatuses;
    runtime.state.runnerFrameMetrics = report.runnerFrameMetrics ?? null;
    runtime.state.worldgenJobFrameMetrics = report.worldgenJobFrameMetrics ?? null;
    runtime.state.lightStatusJobFrameMetrics = report.lightStatusJobFrameMetrics ?? null;
    runtime.state.renderCount = report.renderCount;
    runtime.state.status = "ready";
    runtime.state.lastReport = report;
  }

  applyTargetState(target) {
    if (!target?.ok) {
      return;
    }
    runtime.state.currentTarget = target;
    applyHotbarState(target);
  }

  applyCompileQueueDecision(decision) {
    if (!decision?.ok) {
      return;
    }
    runtime.state.pendingCompileJobCount = decision.pendingCompileJobCount ?? runtime.state.pendingCompileJobCount;
    runtime.state.compileQueued = Boolean(decision.queued);
    if (decision.queued) {
      const center = decisionCenter(decision);
      runtime.state.queuedCompileTargetX = center.centerX;
      runtime.state.queuedCompileTargetZ = center.centerZ;
    } else {
      runtime.state.queuedCompileTargetX = null;
      runtime.state.queuedCompileTargetZ = null;
    }
  }

  async interactBlock(action) {
    if (!this.session) {
      return null;
    }
    try {
      await this.waitForSessionIdle();
      const interaction = await this.withSessionAsync(() => this.session.interactBlock(action));
      if (!interaction?.ok) {
        return null;
      }
      runtime.state.interactionCount += 1;
      runtime.state.lastInteraction = interaction;
      runtime.state.interactionStatus = formatInteractionStatus(interaction);
      applyHotbarState(interaction);
      if (interaction.changed && this.hasRendered) {
        this.requestCameraViewCompile("interaction", { force: true });
      }
      updateDom();
      return interaction;
    } catch (error) {
      runtime.state.ok = false;
      runtime.state.status = stringifyError(error);
      console.error(error);
      updateDom();
      return null;
    }
  }

  selectHotbarSlot(slot) {
    if (!this.session) {
      return false;
    }
    if (this.sessionBusy) {
      setTimeout(() => this.selectHotbarSlot(slot), 0);
      return true;
    }
    const hotbar = this.session.selectHotbarSlot(slot);
    if (!hotbar?.ok) {
      return false;
    }
    applyHotbarState(hotbar);
    updateDom();
    return true;
  }

  adjustCameraSpeed(amount) {
    if (!this.session) {
      return null;
    }
    if (this.sessionBusy) {
      setTimeout(() => this.adjustCameraSpeed(amount), 0);
      return null;
    }
    const camera = this.session.adjustCameraSpeed(amount);
    if (!camera?.ok) {
      return null;
    }
    this.applyCameraState(camera);
    updateDom();
    return camera;
  }

  async withSessionAsync(operation) {
    await this.waitForSessionIdle();
    this.sessionBusy = true;
    try {
      return await operation();
    } finally {
      this.sessionBusy = false;
    }
  }

  async waitForSessionIdle() {
    for (let attempt = 0; this.sessionBusy && attempt < 10_000; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
    if (this.sessionBusy) {
      throw new Error("timed out waiting for WebChunkRenderSession async operation");
    }
  }

  setInputKey(name, down) {
    if (!(name in this.keys)) {
      return false;
    }
    this.keys[name] = Boolean(down);
    return true;
  }

  setTouchKey(name, down) {
    if (!(name in this.touchKeys)) {
      return false;
    }
    this.touchKeys[name] = Boolean(down);
    return true;
  }

  setTouchKeys(keys) {
    for (const [name, down] of Object.entries(keys)) {
      this.setTouchKey(name, down);
    }
  }

  setTouchMovementImpulse(left, forward, active) {
    this.touchMovementImpulse = {
      active: Boolean(active),
      left: sanitizeInputImpulse(left),
      forward: sanitizeInputImpulse(forward),
    };
  }

  currentInputKeys() {
    const keys = defaultInputKeys();
    for (const name of INPUT_KEY_NAMES) {
      keys[name] = Boolean(this.keys[name] || this.touchKeys[name]);
    }
    return keys;
  }

  currentMovementImpulse() {
    return { ...this.touchMovementImpulse };
  }

  clearTouchKeys(names = INPUT_KEY_NAMES) {
    for (const name of names) {
      this.setTouchKey(name, false);
    }
  }

  queueMouseDelta(dx, dy) {
    const x = Number(dx);
    const y = Number(dy);
    if (Number.isFinite(x)) {
      this.mouseDeltaX += x;
    }
    if (Number.isFinite(y)) {
      this.mouseDeltaY += y;
    }
    if (this.pointerDown) {
      this.pointerDown.movement += Math.abs(Number.isFinite(x) ? x : 0)
        + Math.abs(Number.isFinite(y) ? y : 0);
    }
  }

  currentCameraCenter() {
    return {
      centerX: Number(runtime.state.centerX) || 0,
      centerZ: Number(runtime.state.centerZ) || 0,
    };
  }

  recordFrameGap(frameGapMs) {
    if (!Number.isFinite(frameGapMs)) {
      return;
    }
    runtime.state.lastFrameGapMs = frameGapMs;
    runtime.state.maxFrameGapMs = Math.max(runtime.state.maxFrameGapMs, frameGapMs);
    if (this.activeCompileTiming) {
      this.activeCompileTiming.maxFrameGapMs = Math.max(
        this.activeCompileTiming.maxFrameGapMs,
        frameGapMs,
      );
      publishActiveCompileTiming(this.activeCompileTiming);
    }
  }

  startQueuedCompileIfNeeded() {
    if (!this.session || !this.compiler || this.pendingCompile || !runtime.state.ok) {
      return;
    }
    const decision = this.session.takeQueuedCameraRenderCompile(this.pendingCompile);
    this.applyCompileQueueDecision(decision);
    if (!this.hasRendered) {
      updateDom();
      return;
    }
    if (!decision?.startNow) {
      updateDom();
      return;
    }
    setTimeout(() => {
      void this.compileCameraView({
        trigger: decision.trigger ?? "follow-up",
        force: Boolean(decision.force),
        targetCenter: decisionCenter(decision),
        waitForCompletion: false,
      });
    }, 0);
  }

  requestPointerLock() {
    runtime.state.pointerLockAttempted = true;
    if (typeof this.canvas.requestPointerLock === "function") {
      const result = this.canvas.requestPointerLock();
      if (result && typeof result.catch === "function") {
        result.catch(() => {
          runtime.state.pointerLockFallback = true;
          updateDom();
        });
      }
    } else {
      runtime.state.pointerLockFallback = true;
    }
    updateDom();
  }

  updatePointerLockState() {
    runtime.state.pointerLocked = document.pointerLockElement === this.canvas;
    runtime.state.pointerLockFallback =
      runtime.state.pointerLockAttempted && !runtime.state.pointerLocked;
    updateDom();
  }

  syncCanvasSize() {
    if (!this.session) {
      return;
    }
    const rect = this.canvas.getBoundingClientRect();
    const scale = Number.isFinite(window.devicePixelRatio) ? window.devicePixelRatio : 1;
    const width = Math.max(1, Math.round(rect.width * scale));
    const height = Math.max(1, Math.round(rect.height * scale));
    if (width === runtime.state.width && height === runtime.state.height) {
      return;
    }
    const report = this.session.resizeCanvas(width, height);
    if (report?.ok) {
      runtime.state.width = Number(report.width) || width;
      runtime.state.height = Number(report.height) || height;
    }
  }
}

function bindInput(app) {
  window.addEventListener("keydown", (event) => {
    if (isPhysicalKey(event, "KeyN", "n") && !event.repeat) {
      event.preventDefault();
      const camera = app.session?.toggleMovementMode?.();
      if (camera) app.applyCameraState(camera);
      updateDom();
      return;
    }
    const hotbarSlot = hotbarSlotForEvent(event);
    if (hotbarSlot !== null) {
      event.preventDefault();
      if (!event.repeat) {
        app.selectHotbarSlot(hotbarSlot);
      }
      return;
    }
    const key = inputNameForEvent(event);
    if (!key) return;
    event.preventDefault();
    app.setInputKey(key, true);
  });

  window.addEventListener("keyup", (event) => {
    const key = inputNameForEvent(event);
    if (!key) return;
    event.preventDefault();
    app.setInputKey(key, false);
  });

  app.canvas.addEventListener("click", (event) => {
    if (app.touchControls?.shouldIgnoreMouseEvent()) {
      event.preventDefault();
      return;
    }
    app.canvas.focus();
    app.requestPointerLock();
  });

  app.canvas.addEventListener("mousedown", (event) => {
    if (app.touchControls?.shouldIgnoreMouseEvent()) {
      event.preventDefault();
      return;
    }
    app.pointerDragging = true;
    app.pointerDown = {
      button: event.button,
      enabled: runtime.state.pointerLockAttempted,
      movement: 0,
    };
    app.canvas.focus();
    if (event.button === 2) {
      event.preventDefault();
    }
  });

  window.addEventListener("mouseup", (event) => {
    const pointerDown = app.pointerDown;
    app.pointerDragging = false;
    app.pointerDown = null;
    if (!pointerDown?.enabled || pointerDown.button !== event.button || pointerDown.movement > 4) {
      return;
    }
    if (event.button === 0) {
      event.preventDefault();
      app.interactBlock("break");
    } else if (event.button === 2) {
      event.preventDefault();
      app.interactBlock("place");
    }
  });

  app.canvas.addEventListener("contextmenu", (event) => {
    event.preventDefault();
  });

  window.addEventListener("mousemove", (event) => {
    if (app.touchControls?.shouldIgnoreMouseEvent()) {
      return;
    }
    if (document.pointerLockElement === app.canvas) {
      app.queueMouseDelta(event.movementX, event.movementY);
    } else if (app.pointerDragging) {
      app.queueMouseDelta(event.movementX, event.movementY);
    }
  });

  app.canvas.addEventListener("wheel", (event) => {
    event.preventDefault();
    const amount = event.deltaMode === WheelEvent.DOM_DELTA_PIXEL
      ? -event.deltaY * 0.001
      : -event.deltaY * 0.12;
    const camera = app.session?.adjustCameraSpeed?.(amount);
    if (camera) app.applyCameraState(camera);
    updateDom();
  }, { passive: false });

  document.addEventListener("pointerlockchange", () => app.updatePointerLockState());
  document.addEventListener("pointerlockerror", () => {
    runtime.state.pointerLockFallback = true;
    updateDom();
  });
  window.addEventListener("resize", () => {
    app.syncCanvasSize();
    updateDom();
  });
}

function bindHud(app) {
  setHudOpen(defaultHudOpen());
  app.hudToggle?.addEventListener("click", () => setHudOpen(!isHudOpen()));
  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && isHudOpen() && document.pointerLockElement !== app.canvas) {
      setHudOpen(false);
    }
  });
}

class TouchControls {
  constructor(app) {
    this.app = app;
    this.canvas = app.canvas;
    this.root = document.getElementById("touch-controls");
    this.joystick = document.getElementById("touch-joystick");
    this.thumb = document.getElementById("touch-joystick-thumb");
    this.buttons = Array.from(document.querySelectorAll("[data-touch-key]"));
    this.movementPointerId = null;
    this.lookPointerId = null;
    this.movementBaseX = 0;
    this.movementBaseY = 0;
    this.movementThumbX = 0;
    this.movementThumbY = 0;
    this.lookLastX = 0;
    this.lookLastY = 0;
    this.buttonPointers = new Map();
    this.lastTouchAt = 0;

    this.setVisible(hasTouchInput());
    this.bindCanvas();
    this.bindButtons();
    window.addEventListener("blur", () => this.clearAll());
  }

  bindCanvas() {
    this.canvas.addEventListener("pointerdown", (event) => this.onCanvasPointerDown(event), { passive: false });
    this.canvas.addEventListener("pointermove", (event) => this.onCanvasPointerMove(event), { passive: false });
    this.canvas.addEventListener("pointerup", (event) => this.onCanvasPointerEnd(event), { passive: false });
    this.canvas.addEventListener("pointercancel", (event) => this.onCanvasPointerEnd(event), { passive: false });
    this.canvas.addEventListener("lostpointercapture", (event) => this.onCanvasPointerEnd(event), { passive: false });
  }

  bindButtons() {
    for (const button of this.buttons) {
      button.addEventListener("pointerdown", (event) => this.onButtonPointerDown(event), { passive: false });
      button.addEventListener("pointerup", (event) => this.onButtonPointerEnd(event), { passive: false });
      button.addEventListener("pointercancel", (event) => this.onButtonPointerEnd(event), { passive: false });
      button.addEventListener("lostpointercapture", (event) => this.onButtonPointerEnd(event), { passive: false });
    }
  }

  onCanvasPointerDown(event) {
    if (!isTouchPointer(event)) {
      return;
    }
    this.markTouchEvent();
    event.preventDefault();
    this.setVisible(true);
    this.canvas.focus();
    const rect = this.canvas.getBoundingClientRect();
    const onLeft = event.clientX < rect.left + rect.width * 0.5;
    if (onLeft && this.movementPointerId === null) {
      this.startMovement(event);
    } else if (this.lookPointerId === null) {
      this.startLook(event);
    }
  }

  onCanvasPointerMove(event) {
    if (event.pointerId === this.movementPointerId) {
      this.markTouchEvent();
      event.preventDefault();
      this.updateMovement(event.clientX, event.clientY);
    } else if (event.pointerId === this.lookPointerId) {
      this.markTouchEvent();
      event.preventDefault();
      this.updateLook(event.clientX, event.clientY);
    }
  }

  onCanvasPointerEnd(event) {
    if (event.pointerId === this.movementPointerId) {
      this.markTouchEvent();
      event.preventDefault();
      this.clearMovement();
    } else if (event.pointerId === this.lookPointerId) {
      this.markTouchEvent();
      event.preventDefault();
      this.clearLook();
    }
  }

  onButtonPointerDown(event) {
    const key = event.currentTarget?.dataset?.touchKey;
    if (!key) {
      return;
    }
    this.markTouchEvent();
    event.preventDefault();
    event.stopPropagation();
    this.setVisible(true);
    trySetPointerCapture(event.currentTarget, event.pointerId);
    this.buttonPointers.set(event.pointerId, key);
    event.currentTarget.dataset.active = "true";
    this.app.setTouchKey(key, true);
    this.updateRuntimeState();
  }

  onButtonPointerEnd(event) {
    const key = this.buttonPointers.get(event.pointerId);
    if (!key) {
      return;
    }
    this.markTouchEvent();
    event.preventDefault();
    event.stopPropagation();
    this.buttonPointers.delete(event.pointerId);
    const button = this.buttons.find((candidate) => candidate.dataset.touchKey === key);
    if (button) {
      button.dataset.active = "false";
    }
    this.app.setTouchKey(key, false);
    this.updateRuntimeState();
  }

  startMovement(event) {
    this.movementPointerId = event.pointerId;
    this.movementBaseX = event.clientX;
    this.movementBaseY = event.clientY;
    this.movementThumbX = event.clientX;
    this.movementThumbY = event.clientY;
    trySetPointerCapture(this.canvas, event.pointerId);
    this.updateJoystickVisual();
    this.updateMovement(event.clientX, event.clientY);
    this.updateRuntimeState();
  }

  updateMovement(clientX, clientY) {
    const dx = clientX - this.movementBaseX;
    const dy = clientY - this.movementBaseY;
    const distance = Math.hypot(dx, dy);
    const scale = distance > TOUCH_JOYSTICK_MAX_DISTANCE
      ? TOUCH_JOYSTICK_MAX_DISTANCE / distance
      : 1;
    const clampedX = dx * scale;
    const clampedY = dy * scale;
    this.movementThumbX = this.movementBaseX + clampedX;
    this.movementThumbY = this.movementBaseY + clampedY;
    const axisX = clampedX / TOUCH_JOYSTICK_MAX_DISTANCE;
    const axisY = -clampedY / TOUCH_JOYSTICK_MAX_DISTANCE;
    const magnitude = Math.hypot(axisX, axisY);
    const active = magnitude >= TOUCH_AXIS_THRESHOLD;
    let leftImpulse = 0;
    let forwardImpulse = 0;
    if (active) {
      const adjustedMagnitude = (magnitude - TOUCH_AXIS_THRESHOLD) / (1 - TOUCH_AXIS_THRESHOLD);
      const impulseScale = adjustedMagnitude / magnitude;
      leftImpulse = -axisX * impulseScale;
      forwardImpulse = axisY * impulseScale;
    }
    this.app.setTouchMovementImpulse(leftImpulse, forwardImpulse, true);
    this.app.setTouchKeys({
      forward: active && axisY > TOUCH_AXIS_THRESHOLD,
      backward: active && axisY < -TOUCH_AXIS_THRESHOLD,
      left: active && axisX < -TOUCH_AXIS_THRESHOLD,
      right: active && axisX > TOUCH_AXIS_THRESHOLD,
    });
    this.updateJoystickVisual();
    this.updateRuntimeState();
  }

  clearMovement() {
    this.movementPointerId = null;
    this.app.setTouchMovementImpulse(0, 0, false);
    this.app.setTouchKeys({
      forward: false,
      backward: false,
      left: false,
      right: false,
    });
    if (this.joystick) {
      this.joystick.dataset.active = "false";
    }
    this.updateRuntimeState();
  }

  startLook(event) {
    this.lookPointerId = event.pointerId;
    this.lookLastX = event.clientX;
    this.lookLastY = event.clientY;
    trySetPointerCapture(this.canvas, event.pointerId);
    this.updateRuntimeState();
  }

  updateLook(clientX, clientY) {
    this.app.queueMouseDelta(clientX - this.lookLastX, clientY - this.lookLastY);
    this.lookLastX = clientX;
    this.lookLastY = clientY;
    this.updateRuntimeState();
  }

  clearLook() {
    this.lookPointerId = null;
    this.updateRuntimeState();
  }

  clearAll() {
    this.clearMovement();
    this.clearLook();
    for (const pointerId of Array.from(this.buttonPointers.keys())) {
      const key = this.buttonPointers.get(pointerId);
      if (key) {
        this.app.setTouchKey(key, false);
      }
      this.buttonPointers.delete(pointerId);
    }
    for (const button of this.buttons) {
      button.dataset.active = "false";
    }
    this.updateRuntimeState();
  }

  updateJoystickVisual() {
    if (!this.joystick || !this.thumb) {
      return;
    }
    const rect = this.canvas.getBoundingClientRect();
    this.joystick.style.left = `${this.movementBaseX - rect.left}px`;
    this.joystick.style.top = `${this.movementBaseY - rect.top}px`;
    this.joystick.dataset.active = this.movementPointerId === null ? "false" : "true";
    this.thumb.style.left = `${50 + (this.movementThumbX - this.movementBaseX)}px`;
    this.thumb.style.top = `${50 + (this.movementThumbY - this.movementBaseY)}px`;
  }

  setVisible(visible) {
    if (this.root) {
      this.root.dataset.visible = visible ? "true" : "false";
      this.root.setAttribute("aria-hidden", visible ? "false" : "true");
    }
    runtime.state.touchControlsVisible = Boolean(visible);
  }

  markTouchEvent() {
    this.lastTouchAt = performance.now();
  }

  shouldIgnoreMouseEvent() {
    return performance.now() - this.lastTouchAt < 800;
  }

  updateRuntimeState() {
    runtime.state.touchControlsVisible = this.root?.dataset.visible === "true";
    runtime.state.touchJoystickActive = this.movementPointerId !== null;
    runtime.state.touchMovementLeftImpulse = this.app.touchMovementImpulse.left;
    runtime.state.touchMovementForwardImpulse = this.app.touchMovementImpulse.forward;
    runtime.state.touchLookActive = this.lookPointerId !== null;
    runtime.state.touchButtonActiveCount = this.buttonPointers.size;
  }

  snapshot() {
    return {
      visible: this.root?.dataset.visible === "true",
      joystickActive: this.movementPointerId !== null,
      lookActive: this.lookPointerId !== null,
      buttonActiveCount: this.buttonPointers.size,
      keys: { ...this.app.touchKeys },
      movementImpulse: this.app.currentMovementImpulse(),
    };
  }
}

function inputNameForEvent(event) {
  const code = keyboardCode(event);
  switch (code) {
    case "ArrowUp":
    case "KeyW":
      return "forward";
    case "ArrowDown":
    case "KeyS":
      return "backward";
    case "ArrowLeft":
    case "KeyA":
      return "left";
    case "ArrowRight":
    case "KeyD":
      return "right";
    case "Space":
      return "jump";
    case "KeyX":
    case "KeyQ":
      return "descend";
    case "ShiftLeft":
    case "ShiftRight":
      return "shift";
    case "ControlLeft":
    case "ControlRight":
      return "sprint";
    default:
      return code === null ? inputNameForLegacyKey(event.key) : null;
  }
}

function inputNameForLegacyKey(key) {
  switch (key) {
    case "ArrowUp":
    case "w":
    case "W":
      return "forward";
    case "ArrowDown":
    case "s":
    case "S":
      return "backward";
    case "ArrowLeft":
    case "a":
    case "A":
      return "left";
    case "ArrowRight":
    case "d":
    case "D":
      return "right";
    case " ":
    case "Spacebar":
      return "jump";
    case "x":
    case "X":
    case "q":
    case "Q":
      return "descend";
    case "Shift":
      return "shift";
    case "Control":
      return "sprint";
    default:
      return null;
  }
}

function isPhysicalKey(event, code, legacyKey) {
  if (keyboardCode(event) === code) {
    return true;
  }
  return keyboardCode(event) === null && (
    event.key === legacyKey || event.key === legacyKey.toUpperCase()
  );
}

function keyboardCode(event) {
  return typeof event.code === "string" && event.code.length > 0 && event.code !== "Unidentified"
    ? event.code
    : null;
}

async function fetchAssetPack() {
  const response = await fetch(ASSET_PACK_URL);
  if (!response.ok) {
    throw new Error(`failed to fetch ${ASSET_PACK_URL.pathname}: ${response.status} ${response.statusText}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

class RenderSectionWorkerCompiler {
  constructor(assetPack) {
    this.assetPack = assetPack;
    this.pending = new Map();
    this.worker = new Worker(RENDER_COMPILER_WORKER_URL.href, {
      type: "module",
      name: "mclone-render-compiler-app",
    });
    this.worker.onmessage = (event) => {
      const data = event.data ?? {};
      const requestId = Number(data.requestId) || 0;
      const pending = this.pending.get(requestId);
      if (!pending) return;
      this.pending.delete(requestId);
      clearTimeout(pending.timeout);
      const packed = data.packed instanceof Uint8Array ? data.packed : new Uint8Array();
      const packedByteLength = packed.byteLength;
      delete data.packed;
      pending.resolve({
        report: {
          ...data,
          packedByteLength,
        },
        packed,
      });
    };
    this.worker.onerror = (event) => {
      this.rejectAll(new Error(event.message || "render compiler worker failed"));
    };
  }

  compile(request) {
    return new Promise((resolve, reject) => {
      const requestId = Number(request.requestId) || 0;
      if (requestId <= 0) {
        reject(new Error(`invalid render compile request id ${String(request.requestId)}`));
        return;
      }
      const requestAssetPack = this.assetPack.slice();
      const timeout = setTimeout(() => {
        if (!this.pending.has(requestId)) return;
        this.pending.delete(requestId);
        reject(new Error(`timed out waiting for render compiler worker request ${requestId}`));
      }, 20_000);
      this.pending.set(requestId, { resolve, reject, timeout });
      this.worker.postMessage(
        {
          kind: "compile-render-sections",
          requestId,
          bindgenJsUrl: BINDGEN_JS_URL.href,
          bindgenWasmUrl: BINDGEN_WASM_URL.href,
          assetPack: requestAssetPack,
          centerX: request.centerX,
          centerZ: request.centerZ,
          radiusChunks: request.radiusChunks,
        },
        [requestAssetPack.buffer],
      );
    });
  }

  rejectAll(error) {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timeout);
      pending.reject(error);
    }
    this.pending.clear();
  }
}

function centersEqual(left, right) {
  return Number(left?.centerX) === Number(right?.centerX)
    && Number(left?.centerZ) === Number(right?.centerZ);
}

function decisionCenter(decision) {
  const centerX = Number(decision?.centerX);
  const centerZ = Number(decision?.centerZ);
  return {
    centerX: Number.isFinite(centerX) ? centerX : 0,
    centerZ: Number.isFinite(centerZ) ? centerZ : 0,
  };
}

function createCompileTiming({
  sequence,
  trigger,
  targetCenter,
  loadedCenterBefore,
  frameCountBefore,
  renderCountBefore,
}) {
  const now = performance.now();
  return {
    sequence,
    trigger,
    status: "running",
    requestId: null,
    targetCenterX: targetCenter?.centerX ?? null,
    targetCenterZ: targetCenter?.centerZ ?? null,
    loadedCenterBeforeX: loadedCenterBefore?.centerX ?? null,
    loadedCenterBeforeZ: loadedCenterBefore?.centerZ ?? null,
    loadedCenterAfterX: null,
    loadedCenterAfterZ: null,
    startedAtMs: now,
    finishedAtMs: null,
    totalMs: 0,
    beginRequestMs: 0,
    workerRoundTripMs: 0,
    packedByteLength: 0,
    decodeFinishApplyMs: 0,
    chunkViewUpdateCount: 0,
    centerLoaded: false,
    runnerSettled: false,
    submittedCompileSectionCount: 0,
    uploadedSectionCount: 0,
    removedSectionCount: 0,
    acceptedCompileSectionCount: 0,
    staleCompileSectionCount: 0,
    workerSectionCount: 0,
    workerNonEmptySectionCount: 0,
    workerVertexCount: 0,
    workerIndexCount: 0,
    workerFaceCount: 0,
    maxFrameGapMs: 0,
    frameCountBefore,
    frameCountAfter: null,
    renderCountBefore,
    renderCountAfter: null,
    reason: null,
  };
}

function updateCompileTimingFromRequest(timing, request) {
  timing.requestId = Number(request.requestId) || null;
  timing.targetCenterX = Number(request.centerX) || 0;
  timing.targetCenterZ = Number(request.centerZ) || 0;
  timing.submittedCompileSectionCount = Number(request.submittedCompileSectionCount) || 0;
}

function updateCompileTimingFromWait(timing, request, beginStart) {
  timing.beginRequestMs = performance.now() - beginStart;
  const centerX = Number(request.centerX);
  const centerZ = Number(request.centerZ);
  if (Number.isFinite(centerX)) {
    timing.targetCenterX = centerX;
  }
  if (Number.isFinite(centerZ)) {
    timing.targetCenterZ = centerZ;
  }
  if (Number.isFinite(Number(request.updateCountDelta))) {
    timing.chunkViewUpdateCount = (timing.chunkViewUpdateCount || 0)
      + Number(request.updateCountDelta);
  }
  timing.runnerSettled = Boolean(request.runnerSettled);
  timing.centerLoaded = Boolean(request.centerLoaded);
}

function updateCompileTimingFromWorker(timing, report) {
  const summary = report?.summary ?? {};
  timing.workerSectionCount = Number(summary.sectionCount) || 0;
  timing.workerNonEmptySectionCount = Number(summary.nonEmptySectionCount) || 0;
  timing.workerVertexCount = Number(summary.vertexCount) || 0;
  timing.workerIndexCount = Number(summary.indexCount) || 0;
  timing.workerFaceCount = Number(summary.faceCount) || 0;
}

function updateCompileTimingFromReport(timing, report) {
  timing.loadedCenterAfterX = Number(report.centerX) || 0;
  timing.loadedCenterAfterZ = Number(report.centerZ) || 0;
  timing.centerLoaded = true;
  timing.runnerSettled = true;
  timing.uploadedSectionCount = Number(report.uploadedSectionCount) || 0;
  timing.removedSectionCount = Number(report.removedSectionCount) || 0;
  timing.acceptedCompileSectionCount = Number(report.acceptedCompileSectionCount) || 0;
  timing.staleCompileSectionCount = Number(report.staleCompileSectionCount) || 0;
  timing.submittedCompileSectionCount = Number(report.submittedCompileSectionCount)
    || timing.submittedCompileSectionCount;
  timing.packedByteLength = Number(report.workerPackedByteLength) || timing.packedByteLength;
}

function finishCompileTiming(timing, status, reason = null) {
  timing.status = status;
  timing.reason = reason;
  timing.finishedAtMs = performance.now();
  timing.totalMs = timing.finishedAtMs - timing.startedAtMs;
  timing.frameCountAfter = runtime.state.frameCount;
  timing.renderCountAfter = runtime.state.renderCount;
  timing.maxFrameGapMs = Math.max(timing.maxFrameGapMs, runtime.state.lastFrameGapMs || 0);
  publishActiveCompileTiming(timing);
}

function publishActiveCompileTiming(timing) {
  runtime.state.activeCompileTiming = publicCompileTiming(timing);
}

function recordCompileTiming(timing) {
  const snapshot = publicCompileTiming(timing);
  runtime.state.lastCompileTiming = snapshot;
  runtime.state.compileTimings = [...runtime.state.compileTimings, snapshot].slice(-16);
  runtime.state.compileTimingCount += 1;
  runtime.state.activeCompileTiming = null;
}

function publicCompileTiming(timing) {
  const totalMs = timing.finishedAtMs === null
    ? performance.now() - timing.startedAtMs
    : timing.totalMs;
  return {
    sequence: timing.sequence,
    trigger: timing.trigger,
    status: timing.status,
    requestId: timing.requestId,
    targetCenterX: timing.targetCenterX,
    targetCenterZ: timing.targetCenterZ,
    loadedCenterBeforeX: timing.loadedCenterBeforeX,
    loadedCenterBeforeZ: timing.loadedCenterBeforeZ,
    loadedCenterAfterX: timing.loadedCenterAfterX,
    loadedCenterAfterZ: timing.loadedCenterAfterZ,
    totalMs: roundTiming(totalMs),
    beginRequestMs: roundTiming(timing.beginRequestMs),
    workerRoundTripMs: roundTiming(timing.workerRoundTripMs),
    packedByteLength: timing.packedByteLength,
    decodeFinishApplyMs: roundTiming(timing.decodeFinishApplyMs),
    chunkViewUpdateCount: timing.chunkViewUpdateCount,
    centerLoaded: timing.centerLoaded,
    runnerSettled: timing.runnerSettled,
    submittedCompileSectionCount: timing.submittedCompileSectionCount,
    uploadedSectionCount: timing.uploadedSectionCount,
    removedSectionCount: timing.removedSectionCount,
    acceptedCompileSectionCount: timing.acceptedCompileSectionCount,
    staleCompileSectionCount: timing.staleCompileSectionCount,
    workerSectionCount: timing.workerSectionCount,
    workerNonEmptySectionCount: timing.workerNonEmptySectionCount,
    workerVertexCount: timing.workerVertexCount,
    workerIndexCount: timing.workerIndexCount,
    workerFaceCount: timing.workerFaceCount,
    maxFrameGapMs: roundTiming(timing.maxFrameGapMs),
    frameCountBefore: timing.frameCountBefore,
    frameCountAfter: timing.frameCountAfter,
    renderCountBefore: timing.renderCountBefore,
    renderCountAfter: timing.renderCountAfter,
    reason: timing.reason,
  };
}

function roundTiming(value) {
  return Number.isFinite(value) ? Math.round(value * 10) / 10 : 0;
}

function nextAnimationFrame() {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

function updateDom() {
  const state = runtime.state;
  if (state.failed || (state.ready && !state.ok)) {
    setHudOpen(true);
  }
  setText("center", `${state.centerX}, ${state.centerZ}`);
  setText("camera", `${state.cameraX.toFixed(1)}, ${state.cameraY.toFixed(1)}, ${state.cameraZ.toFixed(1)}`);
  setText("mode", state.movementMode);
  setText("slot", String(Number(state.selectedHotbarSlot || 0) + 1));
  setText("ground", state.onGround ? "ground" : state.verticalCollision ? "blocked" : state.horizontalCollision ? "wall" : "air");
  setText("target", formatTarget(state.currentTarget));
  setText("action", state.interactionStatus);
  setText("chunks", String(state.loadedChunkCount));
  setText("sections", String(state.residentSectionCount));
  setText("time", `${Number(state.dayTime || 0).toFixed(0)} / ${Number(state.timeOfDay || 0).toFixed(3)}`);
  setText("actors", `${state.drawnActorCount}/${state.actorCount}`);
  setText("pending", String(state.pendingCompileJobCount));
  setText("compile", formatCompileTiming(state));
  setText("frames", String(state.frameCount));
  setText("lock", state.pointerLocked ? "on" : state.pointerLockFallback ? "fallback" : "off");
  const status = document.getElementById("status");
  if (status) {
    status.textContent = state.status;
    status.dataset.ok = state.ok ? "true" : "false";
    status.dataset.ready = state.ready && state.status === "ready" ? "true" : "false";
  }
}

function formatCompileTiming(state) {
  const timing = state.activeCompileTiming ?? state.lastCompileTiming;
  if (!timing) {
    return "-";
  }
  const label = state.activeCompileTiming ? "active" : timing.status;
  const target = `${timing.targetCenterX},${timing.targetCenterZ}`;
  return `${label} ${target} ${Number(timing.totalMs || 0).toFixed(0)}ms gap ${Number(timing.maxFrameGapMs || 0).toFixed(0)}ms`;
}

function setText(id, value) {
  const element = document.getElementById(id);
  if (element) element.textContent = value;
}

function defaultInputKeys() {
  return Object.fromEntries(INPUT_KEY_NAMES.map((name) => [name, false]));
}

function normalizedDeployAssetVersion() {
  const version = globalThis.__MCLONE_NATIVE_WEB_ASSET_VERSION__;
  if (
    typeof version === "string"
    && version.length > 0
    && version !== "__MCLONE_NATIVE_WEB_ASSET_VERSION__"
  ) {
    return version;
  }
  return null;
}

function versionedUrl(path) {
  const url = new URL(path, import.meta.url);
  if (DEPLOY_ASSET_VERSION !== null) {
    url.searchParams.set("v", DEPLOY_ASSET_VERSION);
  }
  return url;
}

function defaultMovementImpulse() {
  return {
    active: false,
    left: 0,
    forward: 0,
  };
}

function sanitizeInputImpulse(value) {
  const impulse = Number(value);
  if (!Number.isFinite(impulse)) {
    return 0;
  }
  return Math.max(-1, Math.min(1, impulse));
}

function isHudOpen() {
  return document.getElementById("runtime-hud")?.hidden === false;
}

function setHudOpen(open) {
  const hud = document.getElementById("runtime-hud");
  const toggle = document.getElementById("hud-toggle");
  if (hud) {
    hud.hidden = !open;
  }
  if (toggle) {
    toggle.setAttribute("aria-expanded", open ? "true" : "false");
  }
  runtime.state.hudOpen = Boolean(open);
}

function defaultHudOpen() {
  return !hasTouchInput() && window.matchMedia("(min-width: 681px)").matches;
}

function hasTouchInput() {
  return (typeof navigator !== "undefined" && navigator.maxTouchPoints > 0)
    || window.matchMedia("(pointer: coarse)").matches;
}

function isTouchPointer(event) {
  return event.pointerType === "touch" || event.pointerType === "pen";
}

function trySetPointerCapture(target, pointerId) {
  if (!target || typeof target.setPointerCapture !== "function") {
    return;
  }
  try {
    target.setPointerCapture(pointerId);
  } catch (_error) {
    // Some synthetic or browser-generated pointer streams cannot be captured.
  }
}

function formatInteractionStatus(interaction) {
  if (!interaction?.ok) {
    return "idle";
  }
  if (!interaction.hit) {
    return `${interaction.action}: miss`;
  }
  return `${interaction.action}: ${interaction.changed ? "changed" : "same"}`;
}

function applyHotbarState(value) {
  if (!value || typeof value.selectedHotbarSlot === "undefined") {
    return;
  }
  const slot = Number(value.selectedHotbarSlot);
  if (Number.isInteger(slot) && slot >= 0 && slot < 9) {
    runtime.state.selectedHotbarSlot = slot;
  }
}

function hotbarSlotForEvent(event) {
  const code = keyboardCode(event);
  if (code?.startsWith("Digit") || code?.startsWith("Numpad")) {
    const digit = Number(code.slice(-1));
    if (Number.isInteger(digit) && digit >= 1 && digit <= 9) {
      return digit - 1;
    }
    return null;
  }
  return code === null ? hotbarSlotForLegacyKey(event.key) : null;
}

function hotbarSlotForLegacyKey(key) {
  if (key.length !== 1) {
    return null;
  }
  const digit = Number(key);
  if (Number.isInteger(digit) && digit >= 1 && digit <= 9) {
    return digit - 1;
  }
  return null;
}

function formatTarget(interaction) {
  if (!interaction?.ok) {
    return "-";
  }
  if (!interaction.hit) {
    return "miss";
  }
  return `${interaction.blockX}, ${interaction.blockY}, ${interaction.blockZ}`;
}

function snapshotState() {
  return JSON.parse(JSON.stringify(runtime.state));
}

function stringifyError(error) {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}

globalThis.__mcloneNativeAppReady = boot();
