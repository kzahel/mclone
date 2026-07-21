import { existsSync, readdirSync } from "node:fs";

const WAYLAND_SOCKET_PATTERN = /^wayland-\d+$/u;
const WAYLAND_OZONE_ARG = "--ozone-platform=wayland";

/** @param {string | undefined} xdgRuntimeDir */
export function findWaylandSockets(xdgRuntimeDir) {
  if (!xdgRuntimeDir || !existsSync(xdgRuntimeDir)) {
    return [];
  }

  try {
    return readdirSync(xdgRuntimeDir)
      .filter((entry) => WAYLAND_SOCKET_PATTERN.test(entry))
      .sort();
  } catch {
    return [];
  }
}

export function resolveBrowserWebGpuLaunch(env = process.env, platform = process.platform) {
  const xdgRuntimeDir = env.XDG_RUNTIME_DIR ?? "";
  const waylandSockets = platform === "linux" ? findWaylandSockets(xdgRuntimeDir) : [];
  const waylandDisplay = env.WAYLAND_DISPLAY || waylandSockets[0] || "";
  const forceHeadless = env.MCLONE_NATIVE_WEB_FORCE_HEADLESS === "1";
  const useWayland = platform === "linux" && Boolean(waylandDisplay) && !forceHeadless;
  const headed = !forceHeadless && (env.HEADED === "1" || useWayland);
  const configuredArgs = (env.MCLONE_NATIVE_WEB_EXTRA_CHROME_ARGS ?? "")
    .split(/\s+/u)
    .filter(Boolean);
  if (useWayland && !configuredArgs.some((arg) => arg.startsWith("--ozone-platform="))) {
    configuredArgs.push(WAYLAND_OZONE_ARG);
  }

  const browserEnv = {};
  if (useWayland) {
    browserEnv.WAYLAND_DISPLAY = waylandDisplay;
    browserEnv.XDG_SESSION_TYPE = "wayland";
  }

  return {
    autoConfiguredWayland: useWayland && env.HEADED !== "1",
    browserEnv,
    chromeArgs: configuredArgs,
    forceHeadless,
    headed,
    headless: !headed,
    useWayland,
    waylandDisplay,
    waylandSockets,
    suggestedWaylandBrowserEnv: waylandDisplay
      ? {
          CI: "1",
          HEADED: "1",
          WAYLAND_DISPLAY: waylandDisplay,
          XDG_SESSION_TYPE: "wayland",
          MCLONE_NATIVE_WEB_EXTRA_CHROME_ARGS: WAYLAND_OZONE_ARG,
        }
      : undefined,
  };
}
