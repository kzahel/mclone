# Native Vendor Patches

This directory is for temporary patched crates that cannot be expressed through
normal dependency version selection.

- `wgpu-hal-25.0.2`: disables Vulkan imageless framebuffers. On Quest 3 /
  Adreno 740, wgpu-hal 25's imageless framebuffer path leaves true multiview
  attachments zeroed even for a clear-only pass. The workaround is required for
  the Android XR multiview readback proof. Remove it when the workspace moves to
  a wgpu version whose Vulkan framebuffer path passes that proof without a local
  patch.
