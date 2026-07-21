import { useEffect, useRef, useState } from "react";
import type { JSX } from "react";
import type { StructureCatalogEntry } from "../catalog-model";
import {
  StructureViewportController,
  type CameraPreset,
  type ViewportStatus,
} from "../viewport";
import type { StructureRotation, ThemeMode } from "./store";

interface StructureViewerProps {
  boundsVisible: boolean;
  cameraPreset: CameraPreset;
  entry: StructureCatalogEntry;
  hiddenComponents: readonly string[];
  layer: number;
  markersVisible: boolean;
  mirror: boolean;
  rotation: StructureRotation;
  themeMode: ThemeMode;
}

export function StructureViewer(props: StructureViewerProps): JSX.Element {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewportRef = useRef<StructureViewportController | null>(null);
  const [status, setStatus] = useState<ViewportStatus>("loading");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const viewport = new StructureViewportController(host, {
      background: props.themeMode,
      onStatus: (nextStatus, nextError) => {
        setStatus(nextStatus);
        setError(nextError ?? null);
      },
    });
    viewportRef.current = viewport;
    return () => {
      viewport.dispose();
      viewportRef.current = null;
    };
  }, []);

  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    void viewport.load(props.entry).then(() => applyPresentation(viewport, props)).catch(() => {});
  }, [props.entry.structureId]);

  useEffect(() => viewportRef.current?.setTheme(props.themeMode), [props.themeMode]);
  useEffect(() => viewportRef.current?.setCameraPreset(props.cameraPreset), [props.cameraPreset]);
  useEffect(() => viewportRef.current?.setLayer(props.layer), [props.layer]);
  useEffect(() => viewportRef.current?.setMarkersVisible(props.markersVisible), [props.markersVisible]);
  useEffect(() => viewportRef.current?.setBoundsVisible(props.boundsVisible), [props.boundsVisible]);
  useEffect(() => {
    viewportRef.current?.setTransform(props.rotation, props.mirror);
  }, [props.rotation, props.mirror]);
  useEffect(() => {
    const hidden = new Set(props.hiddenComponents);
    for (const component of props.entry.components) {
      viewportRef.current?.setComponentVisible(component.id, !hidden.has(component.id));
    }
  }, [props.entry.structureId, props.hiddenComponents]);

  return (
    <div className="viewportFrame" data-viewer-status={status}>
      <div className="viewportHost" ref={hostRef} />
      {status === "loading" ? (
        <div className="viewportMessage">
          <span className="loadingLeaf" aria-hidden="true">✦</span>
          Baking the view…
        </div>
      ) : null}
      {error ? <div className="viewportMessage errorMessage" role="alert">{error}</div> : null}
      <div className="orientationRose" aria-hidden="true"><span>N</span><i /></div>
      <div className="gestureHint">Drag to orbit · right-drag to pan · scroll to zoom</div>
    </div>
  );
}

function applyPresentation(
  viewport: StructureViewportController,
  props: StructureViewerProps,
): void {
  viewport.setCameraPreset(props.cameraPreset);
  viewport.setLayer(props.layer);
  viewport.setMarkersVisible(props.markersVisible);
  viewport.setBoundsVisible(props.boundsVisible);
  viewport.setTransform(props.rotation, props.mirror);
  const hidden = new Set(props.hiddenComponents);
  for (const component of props.entry.components) {
    viewport.setComponentVisible(component.id, !hidden.has(component.id));
  }
}
