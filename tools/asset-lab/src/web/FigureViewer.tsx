import { useEffect, useRef, useState } from "react";
import type { JSX } from "react";
import type { AnimalCatalogFigure } from "../catalog-model";
import { parseFigureAssetJson } from "../figure-json";
import { FigureViewportController, type CameraPreset } from "../viewport";
import { assetUrl, type ThemeMode } from "./store";

interface FigureViewerProps {
  clipName: string;
  figure: AnimalCatalogFigure;
  onSelectClip: (clipName: string) => void;
  themeMode: ThemeMode;
}

export function FigureViewer({
  clipName,
  figure,
  onSelectClip,
  themeMode,
}: FigureViewerProps): JSX.Element {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewportRef = useRef<FigureViewportController | null>(null);
  const loadedFigureRef = useRef<string | null>(null);
  const [duration, setDuration] = useState(clipDurationFromManifest(figure, clipName));
  const [error, setError] = useState<string | null>(null);
  const [loadStatus, setLoadStatus] = useState<"loading" | "ready" | "error">("loading");
  const [playing, setPlaying] = useState(true);
  const [speed, setSpeed] = useState(1);
  const [time, setTime] = useState(0);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) {
      return;
    }
    const viewport = new FigureViewportController(host, {
      background: viewportBackground(themeMode),
      onTimeChange: setTime,
    });
    viewport.setPlaybackSpeed(speed);
    viewportRef.current = viewport;
    return () => {
      viewport.dispose();
      viewportRef.current = null;
    };
  }, []);

  useEffect(() => {
    viewportRef.current?.setBackground(viewportBackground(themeMode));
  }, [themeMode]);

  useEffect(() => {
    const controller = new AbortController();
    setLoadStatus("loading");
    setError(null);
    loadedFigureRef.current = null;
    void loadSemanticFigure(figure, controller.signal)
      .then((asset) => {
        if (controller.signal.aborted) {
          return;
        }
        const viewport = viewportRef.current;
        if (!viewport) {
          throw new Error("The interactive viewport is unavailable");
        }
        const activeClip = asset.clips[clipName] ? clipName : figure.defaultClip;
        viewport.setAsset(asset, activeClip);
        viewport.setPlaybackSpeed(speed);
        viewport.setPlaying(playing);
        loadedFigureRef.current = figure.name;
        setDuration(viewport.getDuration());
        setTime(0);
        setLoadStatus("ready");
      })
      .catch((loadError: unknown) => {
        if (controller.signal.aborted) {
          return;
        }
        setError(loadError instanceof Error ? loadError.message : String(loadError));
        setLoadStatus("error");
      });
    return () => controller.abort();
  }, [figure.name]);

  useEffect(() => {
    if (loadedFigureRef.current !== figure.name) {
      return;
    }
    const viewport = viewportRef.current;
    viewport?.setClip(clipName);
    if (viewport) {
      setDuration(viewport.getDuration());
      setTime(0);
      viewport.setPlaying(playing);
    }
  }, [clipName, figure.name]);

  const togglePlayback = (): void => {
    const next = !playing;
    setPlaying(next);
    viewportRef.current?.setPlaying(next);
  };

  const scrub = (nextTime: number): void => {
    setPlaying(false);
    viewportRef.current?.setPlaying(false);
    viewportRef.current?.setTime(nextTime);
    setTime(nextTime);
  };

  const changeSpeed = (nextSpeed: number): void => {
    setSpeed(nextSpeed);
    viewportRef.current?.setPlaybackSpeed(nextSpeed);
  };

  const setCamera = (preset: CameraPreset): void => {
    viewportRef.current?.setCameraPreset(preset);
  };

  return (
    <section className="viewerPanel" aria-label={`${figure.label} interactive viewer`}>
      <div className="viewerHeader">
        <div>
          <div className="eyebrow">Canonical figure</div>
          <h2>{figure.label}</h2>
        </div>
        <div className="cameraActions" aria-label="Camera views">
          <button type="button" onClick={() => setCamera("three-quarter")}>3/4</button>
          <button type="button" onClick={() => setCamera("front")}>Front</button>
          <button type="button" onClick={() => setCamera("right")}>Side</button>
          <button type="button" onClick={() => setCamera("top")}>Top</button>
          <button type="button" onClick={() => viewportRef.current?.fit()}>Fit</button>
        </div>
      </div>

      <div className="viewportFrame" data-viewer-status={loadStatus}>
        <div className="viewportHost" ref={hostRef} />
        {loadStatus === "loading" ? <div className="viewportMessage">Loading {figure.label}…</div> : null}
        {error ? <div className="viewportMessage errorMessage">{error}</div> : null}
        <div className="gestureHint">Drag to orbit · right-drag to pan · scroll to zoom</div>
      </div>

      <div className="animationControls" aria-label="Animation controls">
        <button
          className="playButton"
          type="button"
          onClick={togglePlayback}
          disabled={loadStatus !== "ready"}
          aria-label={playing ? "Pause animation" : "Play animation"}
        >
          {playing ? "Pause" : "Play"}
        </button>
        <label className="clipSelect">
          <span>Clip</span>
          <select value={clipName} onChange={(event) => onSelectClip(event.target.value)}>
            {figure.clips.map((clip) => (
              <option key={clip.name} value={clip.name}>{clip.name}</option>
            ))}
          </select>
        </label>
        <label className="timeline">
          <span className="srOnly">Animation time</span>
          <input
            aria-label="Animation time"
            type="range"
            min="0"
            max={Math.max(duration, 0.001)}
            step="0.001"
            value={Math.min(time, duration)}
            disabled={loadStatus !== "ready"}
            onChange={(event) => scrub(Number(event.target.value))}
          />
        </label>
        <output className="timeReadout" aria-live="off">
          {formatSeconds(time)} / {formatSeconds(duration)}
        </output>
        <label className="speedSelect">
          <span>Speed</span>
          <select value={speed} onChange={(event) => changeSpeed(Number(event.target.value))}>
            <option value={0.25}>0.25×</option>
            <option value={0.5}>0.5×</option>
            <option value={1}>1×</option>
            <option value={1.5}>1.5×</option>
            <option value={2}>2×</option>
          </select>
        </label>
      </div>
    </section>
  );
}

async function loadSemanticFigure(
  figure: AnimalCatalogFigure,
  signal: AbortSignal,
): Promise<ReturnType<typeof parseFigureAssetJson>> {
  const response = await fetch(assetUrl(figure.jsonPath), { cache: "no-cache", signal });
  if (!response.ok) {
    throw new Error(`Could not load '${figure.name}': HTTP ${response.status}`);
  }
  const json = await response.text();
  const bytes = new TextEncoder().encode(json);
  if (bytes.byteLength !== figure.semanticBytes) {
    throw new Error(
      `Figure '${figure.name}' byte length drifted: expected ${figure.semanticBytes}, got ${bytes.byteLength}`,
    );
  }
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  const actualSha256 = [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
  if (actualSha256 !== figure.semanticSha256) {
    throw new Error(`Figure '${figure.name}' failed its semantic SHA-256 check`);
  }
  return parseFigureAssetJson(json, figure.jsonPath);
}

function clipDurationFromManifest(figure: AnimalCatalogFigure, clipName: string): number {
  return figure.clips.find((clip) => clip.name === clipName)?.durationSeconds ?? 0;
}

function viewportBackground(themeMode: ThemeMode): string {
  return themeMode === "dark" ? "#1b211e" : "#edf1f0";
}

function formatSeconds(value: number): string {
  return `${value.toFixed(2)}s`;
}
