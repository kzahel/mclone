export interface BrowserWebGpuLaunch {
  autoConfiguredWayland: boolean;
  browserEnv: Record<string, string>;
  chromeArgs: string[];
  forceHeadless: boolean;
  headed: boolean;
  headless: boolean;
  useWayland: boolean;
  waylandDisplay: string;
  waylandSockets: string[];
  suggestedWaylandBrowserEnv:
    | {
        CI: string;
        HEADED: string;
        WAYLAND_DISPLAY: string;
        XDG_SESSION_TYPE: string;
        MCLONE_NATIVE_WEB_EXTRA_CHROME_ARGS: string;
      }
    | undefined;
}

export function findWaylandSockets(xdgRuntimeDir: string | undefined): string[];

export function resolveBrowserWebGpuLaunch(
  env?: NodeJS.ProcessEnv,
  platform?: NodeJS.Platform,
): BrowserWebGpuLaunch;
