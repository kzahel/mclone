import { describe, expect, test } from "vitest";
import {
  readAutoStartWorldEnabled,
  readDebugLaunchEnabled,
} from "../../src/renderer/browser-world-launch-config";

describe("browser world launch config", () => {
  test("auto-starts dedicated server join links", () => {
    expect(readAutoStartWorldEnabled(url("/?server=https://mclone-host.graehlarts.com"))).toBe(true);
  });

  test("lets explicit auto-start false override server join links", () => {
    expect(readAutoStartWorldEnabled(url("/?server=https://mclone-host.graehlarts.com&startWorld=0"))).toBe(false);
  });

  test("keeps debug mode as the legacy auto-start trigger", () => {
    expect(readDebugLaunchEnabled(url("/?mode=debug"))).toBe(true);
    expect(readAutoStartWorldEnabled(url("/?mode=debug"))).toBe(true);
    expect(readAutoStartWorldEnabled(url("/?mode=title"))).toBe(false);
  });
});

function url(path: string): URL {
  return new URL(path, "https://mclone.graehlarts.com/");
}
