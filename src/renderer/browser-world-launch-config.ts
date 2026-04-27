import { hasDedicatedServerJoinParam } from "./browser-world-transport-config";

export function readDebugLaunchEnabled(url: URL): boolean {
  const mode = url.searchParams.get("mode");
  if (mode === "debug") {
    return true;
  }
  if (mode === "title") {
    return false;
  }
  return false;
}

export function readAutoStartWorldEnabled(url: URL): boolean {
  const value = url.searchParams.get("autoStartWorld") ?? url.searchParams.get("startWorld");
  if (value === "1" || value === "true") {
    return true;
  }
  if (value === "0" || value === "false") {
    return false;
  }
  if (hasDedicatedServerJoinParam(url)) {
    return true;
  }
  return readDebugLaunchEnabled(url);
}
