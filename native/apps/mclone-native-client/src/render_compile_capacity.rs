use mclone_frame_budget::{
    RenderCompileCapacityInput, RenderCompileCapacityReport, RenderCompileMeshFootprint,
    RenderCompileThreadReservation, derive_render_compile_capacity,
};

// Applied mode runs before any frame can report its measured mesh footprint.
// Keep this above the measured RD10 pack size so memory only binds on small hosts.
const PREFLIGHT_RENDER_COMPILE_PACK_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RenderCompileCapacityHostKind {
    Flat,
    Xr,
}

impl RenderCompileCapacityHostKind {
    const fn reservation(self) -> RenderCompileThreadReservation {
        match self {
            Self::Flat => RenderCompileThreadReservation::local_integrated_flat(),
            Self::Xr => RenderCompileThreadReservation::local_integrated_xr(),
        }
    }
}

pub(crate) fn preflight_render_compile_capacity_report(
    host_kind: RenderCompileCapacityHostKind,
) -> RenderCompileCapacityReport {
    render_compile_capacity_report(host_kind, preflight_render_compile_mesh_footprint())
}

pub(crate) fn render_compile_capacity_report(
    host_kind: RenderCompileCapacityHostKind,
    mesh_footprint: RenderCompileMeshFootprint,
) -> RenderCompileCapacityReport {
    derive_render_compile_capacity(
        RenderCompileCapacityInput::new(
            std::thread::available_parallelism()
                .ok()
                .map(|value| value.get()),
            host_total_memory_bytes(),
            host_kind.reservation(),
            mesh_footprint,
        )
        .with_floors(
            mclone_app_runtime::render_assets::DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
            mclone_app_runtime::render_assets::DEFAULT_RENDER_SECTION_COMPILE_MAX_PENDING_JOBS,
        ),
    )
}

fn preflight_render_compile_mesh_footprint() -> RenderCompileMeshFootprint {
    RenderCompileMeshFootprint {
        compile_request_bytes: Some(PREFLIGHT_RENDER_COMPILE_PACK_BYTES),
        mesh_vertex_bytes: None,
        mesh_index_bytes: None,
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(crate) fn host_total_memory_bytes() -> Option<u64> {
    let mut value = 0_u64;
    let mut value_len = std::mem::size_of::<u64>();
    let name = b"hw.memsize\0";
    // SAFETY: sysctlbyname reads into a valid u64 buffer and the C string is
    // statically NUL-terminated for the duration of the call.
    let result = unsafe {
        libc::sysctlbyname(
            name.as_ptr().cast(),
            (&mut value as *mut u64).cast(),
            &mut value_len,
            std::ptr::null_mut(),
            0,
        )
    };
    (result == 0 && value_len == std::mem::size_of::<u64>() && value > 0).then_some(value)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn host_total_memory_bytes() -> Option<u64> {
    // SAFETY: sysconf is thread-safe for these read-only process constants and
    // does not dereference caller-owned pointers.
    let pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
    // SAFETY: same as above.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if pages <= 0 || page_size <= 0 {
        return None;
    }
    u64::try_from(pages)
        .ok()
        .zip(u64::try_from(page_size).ok())
        .map(|(pages, page_size)| pages.saturating_mul(page_size))
        .filter(|bytes| *bytes > 0)
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android"
)))]
pub(crate) fn host_total_memory_bytes() -> Option<u64> {
    None
}
