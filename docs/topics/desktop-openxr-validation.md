# Desktop OpenXR Validation

Topic: desktop-openxr-validation

Status: active. Windows VirtualDesktopXR, macOS WiVRn, and Linux WiVRn now
have headset-backed runtime receipts. Linux USB automation was accepted on
Ubuntu 24.04 with WiVRn 26.6.2 and Quest 3 on 2026-07-28. Live three-mode
target switching was accepted on the same Linux/Quest lane on 2026-07-29.

## Scope

This topic owns the current drivable desktop OpenXR runtime lanes: host/runtime
bootstrap, headset connection, bounded clear and real-scene smokes, rendered
evidence, and host-specific failures found before shared XR rendering begins.
It does not own standalone Quest packaging or synthetic stereo.

## Linux WiVRn Lane

The accepted Linux development lane is the official WiVRn server plus its
exact-version Quest client. The native server is preferred when installed;
the official `io.github.wivrn.wivrn` Flatpak is the portable Ubuntu fallback.
The USB smoke deliberately runs WiVRn with encryption disabled, so it does not
depend on an interactive pairing PIN:

```bash
pnpm native:xr:linux:wivrn:check
pnpm native:xr:linux:wivrn:smoke
pnpm native:xr:linux:wivrn:mclone
```

`scripts/start-xr.sh --wivrn-usb`:

- finds `adb` on `PATH` or in the standard Android SDK locations;
- requires an attached authorized Quest and the configured WiVRn package;
- rejects a known Flatpak/client version mismatch before changing headset
  state;
- saves, adjusts, and restores Quest wake/proximity/controller-launch settings;
- installs `adb reverse tcp:9757 tcp:9757`;
- starts or reuses a native or Flatpak `wivrn-server`;
- launches the Quest client through `wivrn+tcp://localhost:9757`;
- waits for the USB connection before launching Mclone; and
- stops only the host it started, removes its temporary reverse tunnel, then
  restores and sleeps the headset.

The Linux default client package is `org.meumeu.wivrn.github`. Override
`QUEST_WIVRN_PACKAGE` for a store or local build. WiVRn requires the host and
client versions to match exactly.

## Vulkan Interop Contract

Mclone creates its OpenXR Vulkan instance at Vulkan 1.1 and wraps the
runtime-created device in `wgpu-hal`. A physical device can advertise Vulkan
1.2+ while a strict loader exposes only the Vulkan 1.1 core entry points.
When `VK_KHR_timeline_semaphore` is available, Mclone must list it explicitly
and tell `wgpu-hal` it is enabled. Otherwise `wgpu-hal` classifies timeline
semaphores as promoted core functionality and can call an absent
`vkGetSemaphoreCounterValue`, which aborts on first submission.

Linux also accepts the versioned system loader name
`libopenxr_loader.so.1`; Ubuntu therefore needs the runtime loader package,
not the development-only unversioned symlink.

## Accepted Linux Evidence

Host and device:

- Ubuntu 24.04, kernel 7.0.0-28;
- AMD Radeon 890M, Mesa RADV 25.2.8;
- official WiVRn Flatpak 26.6.2;
- matching `org.meumeu.wivrn.github` 26.6.2 on Quest 3 over USB; and
- OpenXR loader/runtime discovery through
  `~/.config/openxr/1/active_runtime.json`.

The clear smoke reached `FOCUSED`, created `2064x2162` per-eye swapchains,
submitted two frames, drained `STOPPING -> IDLE -> EXITING`, and exited
successfully. The real Mclone smoke submitted 120 frames and reported terrain
sections, indices, and two actors from the shared scene host. A longer
900-frame run produced a `4128x2208` Quest screencap with textured, non-black
stereo pixels in both eyes. The unattended headset was lying down and the
captured view was inside leaf geometry, so this proves stereo pixel submission
but is not representative composition acceptance.

The product run verb also passed a bounded
`--desktop-xr --no-window --frames 120` lifecycle check, including the shared
title UI and graceful OpenXR shutdown.

On 2026-07-29, a headset-worn persistent `--desktop-xr --no-window` run
opened through the expected product-default title menu. The user opened a
world from that menu and accepted the resulting stereo overworld composition
and rendering on Quest 3. Turning off the headset display left the host
running as expected; terminating the launcher afterward restored the Quest
settings and removed the temporary USB tunnel.

The subsequent full-scene target-manager cycle submitted 1,000 frames and
entered through the shared UI action seam:

```text
dual per-eye
  -> array per-eye
  -> array multiview
  -> array per-eye
  -> dual per-eye
```

The dual and stereo-array target families each reported `142,795,776` active
bytes at the runtime's `2064x2162` eye extent. Desktop uses bounded
retire-first recreation because WiVRn did not accept overlapping old and
replacement swapchain families. Topology rebuilds took `3.475ms` and
`5.054ms`; same-array encoding changes took `0.001ms`. Every commit reported
zero outstanding acquired images, no inactive target family survived, and the
world/OpenXR session remained continuous through frame 1,000.

The shared Vulkan wrapper now treats OpenXR swapchain images as externally
owned when constructing `wgpu` textures. Without the no-op external ownership
handler, destruction of the retired wrapper could destroy a runtime-owned
`VkImage` and crash WiVRn during topology changes.

## Remaining Acceptance

- Validate the persistent companion-window verb in a real Linux desktop
  session. The persistent windowless product flow and representative
  headset-worn overworld composition pass.
- Secure Wi-Fi pairing is a normal WiVRn dashboard workflow and is not covered
  by the no-encryption USB smoke.
- Keep post-refactor tracked-controller and ordinary-gamepad hardware
  acceptance separate from runtime liveness.
