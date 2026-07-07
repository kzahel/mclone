use mclone_frame_budget::{
    RenderCompileCapacityInput, RenderCompileCapacityReport, RenderCompileMeshFootprint,
    RenderCompileThreadReservation, derive_render_compile_capacity,
};

// Applied mode runs before any frame can report its measured mesh footprint.
// Keep this above the measured RD10 pack size so memory only binds on small hosts.
const PREFLIGHT_RENDER_COMPILE_PACK_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderCompileCapacityHostKind {
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

pub fn preflight_render_compile_capacity_report(
    host_kind: RenderCompileCapacityHostKind,
    total_memory_bytes: Option<u64>,
) -> RenderCompileCapacityReport {
    render_compile_capacity_report(
        host_kind,
        total_memory_bytes,
        preflight_render_compile_mesh_footprint(),
    )
}

pub fn render_compile_capacity_report(
    host_kind: RenderCompileCapacityHostKind,
    total_memory_bytes: Option<u64>,
    mesh_footprint: RenderCompileMeshFootprint,
) -> RenderCompileCapacityReport {
    render_compile_capacity_report_from_inputs(
        host_kind,
        std::thread::available_parallelism()
            .ok()
            .map(|value| value.get()),
        total_memory_bytes,
        mesh_footprint,
    )
}

pub fn render_compile_capacity_report_from_inputs(
    host_kind: RenderCompileCapacityHostKind,
    available_parallelism: Option<usize>,
    total_memory_bytes: Option<u64>,
    mesh_footprint: RenderCompileMeshFootprint,
) -> RenderCompileCapacityReport {
    derive_render_compile_capacity(
        RenderCompileCapacityInput::new(
            available_parallelism,
            total_memory_bytes,
            host_kind.reservation(),
            mesh_footprint,
        )
        .with_floors(
            crate::DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
            crate::DEFAULT_RENDER_SECTION_COMPILE_MAX_PENDING_JOBS,
        ),
    )
}

pub fn preflight_render_compile_mesh_footprint() -> RenderCompileMeshFootprint {
    RenderCompileMeshFootprint {
        compile_request_bytes: Some(PREFLIGHT_RENDER_COMPILE_PACK_BYTES),
        mesh_vertex_bytes: None,
        mesh_index_bytes: None,
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn host_total_memory_bytes() -> Option<u64> {
    parse_proc_meminfo_total_memory_bytes(&std::fs::read_to_string("/proc/meminfo").ok()?)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn host_total_memory_bytes() -> Option<u64> {
    None
}

#[cfg(any(target_os = "linux", target_os = "android", test))]
fn parse_proc_meminfo_total_memory_bytes(contents: &str) -> Option<u64> {
    contents.lines().find_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix("MemTotal:")?.trim();
        let kib = rest.split_whitespace().next()?.parse::<u64>().ok()?;
        kib.checked_mul(1024)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proc_meminfo_total_memory_parser_reads_memtotal_kib() {
        assert_eq!(
            parse_proc_meminfo_total_memory_bytes("MemFree: 100 kB\nMemTotal:       16384000 kB\n"),
            Some(16_777_216_000)
        );
    }

    #[test]
    fn render_compile_capacity_preflight_uses_shared_floors() {
        let report = render_compile_capacity_report_from_inputs(
            RenderCompileCapacityHostKind::Flat,
            None,
            Some(1024 * 1024 * 1024),
            preflight_render_compile_mesh_footprint(),
        );
        assert_eq!(
            report.derived_worker_count,
            crate::DEFAULT_RENDER_SECTION_COMPILE_WORKERS
        );
        assert_eq!(
            report.derived_max_pending_jobs,
            crate::DEFAULT_RENDER_SECTION_COMPILE_MAX_PENDING_JOBS
        );
    }
}
