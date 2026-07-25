import { useEffect, useRef } from "react";
import type {
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent as ReactPointerEvent,
  RefObject,
} from "react";
import {
  TerrainLabNavigationSession,
  type TerrainLabNavigationUpdate,
} from "../../generated/pkg/mclone_terrain_lab";

import type {
  TerrainLabCamera,
  TerrainLabState,
} from "../state";
import { initializeTerrainLab } from "./terrain-lab-wasm";

export interface TerrainLabNavigationViewport {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface NavigationFacts {
  centerX: number;
  centerZ: number;
  blocksAcross: number;
  yaw: number;
  pitch: number;
  viewChanged: boolean;
  signal: string;
}

interface PendingNavigation {
  state: TerrainLabState | undefined;
  camera: TerrainLabCamera | undefined;
}

interface UseWorldViewNavigationOptions {
  stageRef: RefObject<HTMLDivElement | null>;
  enabled: boolean;
  state: TerrainLabState;
  camera: TerrainLabCamera;
  resolveViewport: (
    clientX: number,
    clientY: number,
  ) => TerrainLabNavigationViewport | undefined;
  onStateChange: (state: TerrainLabState) => void;
  onCameraChange: (camera: TerrainLabCamera) => void;
  onTap?: (clientX: number, clientY: number) => void;
}

export function useWorldViewNavigation({
  stageRef,
  enabled,
  state,
  camera,
  resolveViewport,
  onStateChange,
  onCameraChange,
  onTap,
}: UseWorldViewNavigationOptions): {
  onPointerDown: (event: ReactPointerEvent<HTMLDivElement>) => void;
  onPointerMove: (event: ReactPointerEvent<HTMLDivElement>) => void;
  onPointerUp: (event: ReactPointerEvent<HTMLDivElement>) => void;
  onPointerCancel: (event: ReactPointerEvent<HTMLDivElement>) => void;
  onKeyDown: (event: ReactKeyboardEvent<HTMLDivElement>) => void;
} {
  const sessionRef = useRef<TerrainLabNavigationSession | undefined>(undefined);
  const frameRef = useRef(0);
  const pendingRef = useRef<PendingNavigation | undefined>(undefined);
  const currentRef = useRef({
    state,
    camera,
    resolveViewport,
    onStateChange,
    onCameraChange,
    onTap,
  });
  currentRef.current = {
    state,
    camera,
    resolveViewport,
    onStateChange,
    onCameraChange,
    onTap,
  };

  useEffect(() => {
    if (!enabled) {
      return;
    }
    const session = new TerrainLabNavigationSession();
    sessionRef.current = session;
    syncNavigationSession(session, currentRef.current.state, currentRef.current.camera);
    return () => {
      if (frameRef.current !== 0) {
        cancelAnimationFrame(frameRef.current);
        frameRef.current = 0;
      }
      pendingRef.current = undefined;
      if (sessionRef.current === session) {
        sessionRef.current = undefined;
      }
      session.free();
    };
  }, [enabled]);

  const publish = (facts: NavigationFacts, immediate = false): void => {
    if (facts.viewChanged) {
      const current = currentRef.current;
      const nextState = {
        ...current.state,
        centerX: facts.centerX,
        centerZ: facts.centerZ,
        blocksAcross: facts.blocksAcross,
      };
      const nextCamera = {
        yaw: facts.yaw,
        pitch: facts.pitch,
      };
      pendingRef.current = {
        state: navigationStateEqual(current.state, nextState) ? undefined : nextState,
        camera: navigationCameraEqual(current.camera, nextCamera) ? undefined : nextCamera,
      };
    }
    if (immediate) {
      flushPendingNavigation(
        pendingRef,
        frameRef,
        currentRef.current.onStateChange,
        currentRef.current.onCameraChange,
      );
      return;
    }
    if (frameRef.current === 0 && pendingRef.current) {
      frameRef.current = requestAnimationFrame(() => {
        flushPendingNavigation(
          pendingRef,
          frameRef,
          currentRef.current.onStateChange,
          currentRef.current.onCameraChange,
        );
      });
    }
  };

  const onPointerDown = (
    event: ReactPointerEvent<HTMLDivElement>,
  ): void => {
    if (event.button !== 0 && event.button !== 1 && event.button !== 2) {
      return;
    }
    event.preventDefault();
    event.currentTarget.focus({ preventScroll: true });
    event.currentTarget.setPointerCapture(event.pointerId);
    const session = sessionRef.current;
    const viewport = currentRef.current.resolveViewport(event.clientX, event.clientY);
    if (!session || !viewport) {
      return;
    }
    syncNavigationSession(
      session,
      currentRef.current.state,
      currentRef.current.camera,
    );
    const purpose =
      currentRef.current.state.view === "3d"
      && event.button === 0
      && !event.shiftKey
        ? "orbit"
        : "pan";
    consumeUpdate(session.pointerDown(
      event.pointerId,
      viewport.x,
      viewport.y,
      purpose,
      performance.now() / 1_000,
      viewport.width,
      viewport.height,
    ));
  };

  const onPointerMove = (
    event: ReactPointerEvent<HTMLDivElement>,
  ): void => {
    const session = sessionRef.current;
    const viewport = currentRef.current.resolveViewport(event.clientX, event.clientY);
    if (!session || !viewport) {
      return;
    }
    publish(consumeUpdate(session.pointerMove(
      event.pointerId,
      viewport.x,
      viewport.y,
      performance.now() / 1_000,
      viewport.width,
      viewport.height,
    )));
  };

  const onPointerUp = (
    event: ReactPointerEvent<HTMLDivElement>,
  ): void => {
    const session = sessionRef.current;
    const viewport = currentRef.current.resolveViewport(event.clientX, event.clientY);
    if (!session || !viewport) {
      return;
    }
    const facts = consumeUpdate(session.pointerUp(
      event.pointerId,
      viewport.x,
      viewport.y,
      performance.now() / 1_000,
      viewport.width,
      viewport.height,
    ));
    publish(facts, true);
    if (
      event.button === 0
      && (facts.signal === "tap" || facts.signal === "double-tap")
    ) {
      currentRef.current.onTap?.(event.clientX, event.clientY);
    }
  };

  const onPointerCancel = (
    _event: ReactPointerEvent<HTMLDivElement>,
  ): void => {
    const session = sessionRef.current;
    if (!session) {
      return;
    }
    publish(consumeUpdate(session.cancel()), true);
  };

  const onKeyDown = (
    event: ReactKeyboardEvent<HTMLDivElement>,
  ): void => {
    const session = sessionRef.current;
    if (!session || !event.key.startsWith("Arrow")) {
      return;
    }
    syncNavigationSession(
      session,
      currentRef.current.state,
      currentRef.current.camera,
    );
    const facts = consumeUpdate(session.arrow(event.key));
    if (!facts.viewChanged) {
      return;
    }
    event.preventDefault();
    publish(facts, true);
  };

  useEffect(() => {
    if (!enabled) {
      return;
    }
    const stage = stageRef.current;
    if (!stage) {
      return;
    }
    const wheel = (event: WheelEvent): void => {
      const session = sessionRef.current;
      const viewport = currentRef.current.resolveViewport(
        event.clientX,
        event.clientY,
      );
      if (!session || !viewport) {
        return;
      }
      event.preventDefault();
      syncNavigationSession(
        session,
        currentRef.current.state,
        currentRef.current.camera,
      );
      const map = currentRef.current.state.view === "map";
      publish(consumeUpdate(session.wheel(
        event.deltaY * 0.0015,
        map ? viewport.x / Math.max(viewport.width, 1) - 0.5 : 0,
        map ? viewport.y / Math.max(viewport.height, 1) - 0.5 : 0,
        viewport.width,
        viewport.height,
      )), true);
    };
    stage.addEventListener("wheel", wheel, { passive: false });
    return () => stage.removeEventListener("wheel", wheel);
  }, [enabled, stageRef]);

  return {
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPointerCancel,
    onKeyDown,
  };
}

export async function panTerrainLabByFraction(
  state: TerrainLabState,
  camera: TerrainLabCamera,
  horizontal: number,
  depth: number,
  fraction: number,
): Promise<TerrainLabState> {
  await initializeTerrainLab();
  const session = new TerrainLabNavigationSession();
  try {
    syncNavigationSession(session, state, camera);
    const facts = consumeUpdate(session.panFraction(horizontal, depth, fraction));
    return {
      ...state,
      centerX: facts.centerX,
      centerZ: facts.centerZ,
      blocksAcross: facts.blocksAcross,
    };
  } finally {
    session.free();
  }
}

export async function zoomTerrainLabByFactor(
  state: TerrainLabState,
  camera: TerrainLabCamera,
  factor: number,
): Promise<TerrainLabState> {
  await initializeTerrainLab();
  const session = new TerrainLabNavigationSession();
  try {
    syncNavigationSession(session, state, camera);
    const facts = consumeUpdate(session.zoomFactor(factor));
    return {
      ...state,
      centerX: facts.centerX,
      centerZ: facts.centerZ,
      blocksAcross: facts.blocksAcross,
    };
  } finally {
    session.free();
  }
}

function syncNavigationSession(
  session: TerrainLabNavigationSession,
  state: TerrainLabState,
  camera: TerrainLabCamera,
): void {
  session.sync(
    state.centerX,
    state.centerZ,
    state.blocksAcross,
    state.view,
    state.projection,
    camera.yaw,
    camera.pitch,
  );
}

function consumeUpdate(update: TerrainLabNavigationUpdate): NavigationFacts {
  try {
    return {
      centerX: update.centerX,
      centerZ: update.centerZ,
      blocksAcross: update.blocksAcross,
      yaw: update.yawRadians,
      pitch: update.pitchRadians,
      viewChanged: update.viewChanged,
      signal: update.signal,
    };
  } finally {
    update.free();
  }
}

function flushPendingNavigation(
  pendingRef: { current: PendingNavigation | undefined },
  frameRef: { current: number },
  onStateChange: (state: TerrainLabState) => void,
  onCameraChange: (camera: TerrainLabCamera) => void,
): void {
  if (frameRef.current !== 0) {
    cancelAnimationFrame(frameRef.current);
    frameRef.current = 0;
  }
  const pending = pendingRef.current;
  pendingRef.current = undefined;
  if (pending?.state) {
    onStateChange(pending.state);
  }
  if (pending?.camera) {
    onCameraChange(pending.camera);
  }
}

function navigationStateEqual(
  left: TerrainLabState,
  right: TerrainLabState,
): boolean {
  return left.centerX === right.centerX
    && left.centerZ === right.centerZ
    && left.blocksAcross === right.blocksAcross;
}

function navigationCameraEqual(
  left: TerrainLabCamera,
  right: TerrainLabCamera,
): boolean {
  return left.yaw === right.yaw && left.pitch === right.pitch;
}
