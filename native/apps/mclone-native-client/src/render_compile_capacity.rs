use mclone_app_runtime::render_compile_capacity::{
    preflight_render_compile_capacity_report as shared_preflight_report,
    render_compile_capacity_report as shared_capacity_report,
};
use mclone_frame_budget::{RenderCompileCapacityReport, RenderCompileMeshFootprint};

pub(crate) use mclone_app_runtime::render_compile_capacity::RenderCompileCapacityHostKind;

pub(crate) fn preflight_render_compile_capacity_report(
    host_kind: RenderCompileCapacityHostKind,
) -> RenderCompileCapacityReport {
    shared_preflight_report(host_kind, host_total_memory_bytes())
}

pub(crate) fn render_compile_capacity_report(
    host_kind: RenderCompileCapacityHostKind,
    mesh_footprint: RenderCompileMeshFootprint,
) -> RenderCompileCapacityReport {
    shared_capacity_report(host_kind, host_total_memory_bytes(), mesh_footprint)
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
