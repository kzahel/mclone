//! `XR_META_performance_metrics` probe for standalone Quest / Android XR.
//!
//! Opt-in on-device diagnostic. When the Oculus runtime exposes the extension
//! this enumerates the available performance-metric counter paths, enables
//! collection, then after a steady-state warmup queries every counter and logs
//! the values/units. The default `--perf-metrics` probe is one-shot and
//! disables collection after the first valid sample; `--perf-metrics-periodic`
//! keeps sampling for longer perf lanes.
//!
//! The point is to split a heavy frame (e.g. the RD10 frozen-render lane) into
//! the buckets the app's own `Instant`-based timers cannot see:
//!   - `app/gpu_frametime`        -> GPU shader/raster/fill/depth cost
//!   - `app/cpu_frametime`        -> CPU traversal/cull/draw-submission cost
//!   - `compositor/gpu_frametime`, `dropped_frame_count` -> compositor / pacing
//!   - `device/gpu_utilization`, `device/cpu_utilization_*` -> headroom
//!
//! These are measured by the runtime/compositor itself, so unlike the per-eye
//! wall-clock timers (which `device.poll(Wait)` already conflates with GPU
//! execution) they attribute cost to the GPU and compositor directly.
//!
//! Gated behind `--perf-metrics` / `--perf-metrics-periodic`; ordinary launches
//! never enable collection or emit any of these markers. Modelled on the Playbox
//! reference probe.
#![allow(unsafe_code)]

use std::ptr;
use std::time::Instant;

use openxr as xr;

/// Frames to render before the first sample so the runtime has a steady-state
/// window to measure. At ~62-72 Hz this lands inside the perf sample window.
const WARMUP_FRAMES: u32 = 720;
/// Frames to wait between sampling attempts when no counter has a value yet.
const RETRY_FRAMES: u32 = 120;
/// Frames to wait between successful periodic samples.
const PERIODIC_SAMPLE_INTERVAL_FRAMES: u32 = 180;
/// Give up after this many attempts so a runtime that never populates the
/// counters does not keep the collection state enabled forever.
const MAX_ATTEMPTS: u32 = 8;
/// Untimed sweeps to prime the counters before the timed read.
const WARMUP_SWEEPS: u32 = 1;
/// Timed sweeps; the last sweep's values are the ones reported.
const TIMED_SWEEPS: u32 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum XrPerformanceMetricsMode {
    OneShot,
    Periodic,
}

impl XrPerformanceMetricsMode {
    fn label(self) -> &'static str {
        match self {
            Self::OneShot => "one-shot",
            Self::Periodic => "periodic",
        }
    }

    fn is_periodic(self) -> bool {
        matches!(self, Self::Periodic)
    }
}

/// Opt-in probe holding the loaded extension function pointers and the
/// enumerated counter paths. Self-contained after construction (it copies the
/// raw function-pointer table and session/path handles, so it does not borrow
/// the `xr::Instance`).
pub(super) struct XrPerformanceMetricsProbe {
    fp: xr::raw::PerformanceMetricsMETA,
    session: xr::sys::Session,
    counters: Vec<(xr::sys::Path, String)>,
    last_dropped_frames: Option<f64>,
    perf_window_dropped_frames_start: Option<f64>,
    perf_window_started: bool,
    frames: u32,
    next_attempt_frame: u32,
    attempts: u32,
    samples_logged: u32,
    mode: XrPerformanceMetricsMode,
    done: bool,
}

impl XrPerformanceMetricsProbe {
    /// Build the probe if the extension was enabled and exposes counters.
    /// Returns `None` (after logging) when the runtime cannot satisfy the
    /// request, so callers can treat the probe as best-effort.
    pub(super) fn new<G: xr::Graphics>(
        instance: &xr::Instance,
        session: &xr::Session<G>,
        mode: XrPerformanceMetricsMode,
    ) -> Option<Self> {
        let fp = *instance.exts().meta_performance_metrics.as_ref()?;
        let counters = match enumerate_counters(instance, &fp) {
            Ok(counters) => counters,
            Err(err) => {
                log::warn!("XR_META_performance_metrics: counter enumeration failed: {err:?}");
                return None;
            }
        };
        if counters.is_empty() {
            log::warn!(
                "XR_META_performance_metrics: extension present but enumerated 0 counter paths"
            );
            return None;
        }
        log::info!(
            "XR_META_performance_metrics available: {} counters: {}",
            counters.len(),
            counters
                .iter()
                .map(|(_, name)| name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        let session_handle = session.as_raw();
        if let Err(err) = set_metrics_enabled(&fp, session_handle, true) {
            log::warn!("XR_META_performance_metrics: enabling collection failed: {err:?}");
            return None;
        }
        log::info!(
            "XR_META_performance_metrics: collection enabled, mode={}, warming up",
            mode.label()
        );
        let initial_dropped_frames =
            query_named_counter_scalar(&fp, session_handle, &counters, "dropped_frame");
        Some(Self {
            fp,
            session: session_handle,
            counters,
            last_dropped_frames: initial_dropped_frames,
            perf_window_dropped_frames_start: None,
            perf_window_started: false,
            frames: 0,
            next_attempt_frame: WARMUP_FRAMES,
            attempts: 0,
            samples_logged: 0,
            mode,
            done: false,
        })
    }

    pub(super) fn start_perf_window(&mut self) {
        let dropped_frames =
            query_named_counter_scalar(&self.fp, self.session, &self.counters, "dropped_frame");
        self.last_dropped_frames = dropped_frames;
        self.perf_window_dropped_frames_start = dropped_frames;
        self.perf_window_started = true;
        self.frames = 0;
        self.next_attempt_frame = WARMUP_FRAMES;
        self.attempts = 0;
        self.samples_logged = 0;
        log::info!(
            "MCLONE_ANDROID_XR_PERF_METRICS_WINDOW_START dropped_frames_start={} counters={} mode={} interval_frames={}",
            format_metric(dropped_frames),
            self.counters.len(),
            self.mode.label(),
            PERIODIC_SAMPLE_INTERVAL_FRAMES
        );
    }

    pub(super) const fn perf_window_started(&self) -> bool {
        self.perf_window_started
    }

    /// Advance one rendered frame. Once warmed up, samples every counter and
    /// emits the marker block. Cheap and a no-op after one-shot completion.
    pub(super) fn tick(&mut self) {
        if self.done {
            return;
        }
        self.frames += 1;
        if self.frames < self.next_attempt_frame {
            return;
        }
        self.attempts += 1;
        let any_valid = self.sample_and_log();
        if any_valid {
            self.samples_logged += 1;
            if self.mode.is_periodic() {
                self.attempts = 0;
                self.next_attempt_frame = self.frames + PERIODIC_SAMPLE_INTERVAL_FRAMES;
            } else {
                self.disable_and_finish("probe complete");
            }
        } else if self.attempts >= MAX_ATTEMPTS {
            self.disable_and_finish("probe gave up without a valid sample");
        } else {
            self.next_attempt_frame = self.frames + RETRY_FRAMES;
        }
    }

    fn disable_and_finish(&mut self, reason: &str) {
        if let Err(err) = set_metrics_enabled(&self.fp, self.session, false) {
            log::warn!("XR_META_performance_metrics: disabling collection failed: {err:?}");
        } else {
            log::info!("XR_META_performance_metrics: {reason}, collection disabled");
        }
        self.done = true;
    }

    fn sample_and_log(&mut self) -> bool {
        for _ in 0..WARMUP_SWEEPS {
            for (path, _) in &self.counters {
                let _ = query_counter(&self.fp, self.session, *path);
            }
        }

        let mut latest: Vec<Option<xr::sys::PerformanceMetricsCounterMETA>> =
            vec![None; self.counters.len()];
        let start = Instant::now();
        let mut query_count = 0u32;
        for sweep in 0..TIMED_SWEEPS {
            for (index, (path, _)) in self.counters.iter().enumerate() {
                let counter = query_counter(&self.fp, self.session, *path);
                query_count += 1;
                if sweep == TIMED_SWEEPS - 1 {
                    latest[index] = counter;
                }
            }
        }
        let elapsed = start.elapsed();
        let per_query_us = elapsed.as_secs_f64() * 1e6 / f64::from(query_count.max(1));

        let mut sample = PerfMetricsSample::default();
        let mut any_valid = false;
        for ((_, name), counter) in self.counters.iter().zip(latest.iter()) {
            match counter {
                Some(counter) => match counter_scalar(counter) {
                    Some(value) => {
                        any_valid = true;
                        sample.assign(name, value);
                        log::info!(
                            "MCLONE_ANDROID_XR_PERF_METRICS_COUNTER name={} value={:.3} unit={}",
                            name,
                            value,
                            unit_label(counter.counter_unit)
                        );
                    }
                    None => log::info!(
                        "MCLONE_ANDROID_XR_PERF_METRICS_COUNTER name={name} value=<no value yet>"
                    ),
                },
                None => log::info!(
                    "MCLONE_ANDROID_XR_PERF_METRICS_COUNTER name={name} value=<query failed>"
                ),
            }
        }

        let dropped_frames_start = self.last_dropped_frames;
        let dropped_frames_delta =
            metric_counter_delta(dropped_frames_start, sample.dropped_frames);
        let perf_window_dropped_frames_delta =
            metric_counter_delta(self.perf_window_dropped_frames_start, sample.dropped_frames);
        if sample.dropped_frames.is_some() {
            self.last_dropped_frames = sample.dropped_frames;
        }

        log::info!(
            "MCLONE_ANDROID_XR_PERF_METRICS app_gpu_ms={} app_cpu_ms={} compositor_gpu_ms={} compositor_cpu_ms={} gpu_util_pct={} cpu_util_avg_pct={} cpu_util_worst_pct={} motion_to_photon_ms={} dropped_frames={} dropped_frames_start={} dropped_frames_end={} dropped_frames_delta={} perf_window_active={} perf_window_dropped_frames_start={} perf_window_dropped_frames_delta={} stale_frames={} counters={} any_valid={} attempt={}/{} per_query_us={:.2} mode={} sample={} interval_frames={}",
            format_metric(sample.app_gpu_ms),
            format_metric(sample.app_cpu_ms),
            format_metric(sample.compositor_gpu_ms),
            format_metric(sample.compositor_cpu_ms),
            format_metric(sample.gpu_util_pct),
            format_metric(sample.cpu_util_avg_pct),
            format_metric(sample.cpu_util_worst_pct),
            format_metric(sample.motion_to_photon_ms),
            format_metric(sample.dropped_frames),
            format_metric(dropped_frames_start),
            format_metric(sample.dropped_frames),
            format_metric(dropped_frames_delta),
            self.perf_window_started,
            format_metric(self.perf_window_dropped_frames_start),
            format_metric(perf_window_dropped_frames_delta),
            format_metric(sample.stale_frames),
            self.counters.len(),
            any_valid,
            self.attempts,
            MAX_ATTEMPTS,
            per_query_us,
            self.mode.label(),
            self.samples_logged + 1,
            PERIODIC_SAMPLE_INTERVAL_FRAMES
        );
        any_valid
    }
}

/// Key counters pulled out of the enumerated set into the compact marker. The
/// full set is always logged per-counter, so a path the matcher misses is
/// still captured in the inventory lines.
#[derive(Default)]
struct PerfMetricsSample {
    app_gpu_ms: Option<f64>,
    app_cpu_ms: Option<f64>,
    compositor_gpu_ms: Option<f64>,
    compositor_cpu_ms: Option<f64>,
    gpu_util_pct: Option<f64>,
    cpu_util_avg_pct: Option<f64>,
    cpu_util_worst_pct: Option<f64>,
    motion_to_photon_ms: Option<f64>,
    dropped_frames: Option<f64>,
    stale_frames: Option<f64>,
}

impl PerfMetricsSample {
    fn assign(&mut self, name: &str, value: f64) {
        // Match on distinctive substrings so prefix/spelling differences across
        // runtime versions still slot the standard Meta counter paths.
        if name.contains("app/gpu") {
            self.app_gpu_ms = Some(value);
        } else if name.contains("app/cpu") {
            self.app_cpu_ms = Some(value);
        } else if name.contains("compositor/gpu") {
            self.compositor_gpu_ms = Some(value);
        } else if name.contains("compositor/cpu") {
            self.compositor_cpu_ms = Some(value);
        } else if name.contains("motion_to_photon") {
            self.motion_to_photon_ms = Some(value);
        } else if name.contains("dropped_frame") {
            self.dropped_frames = Some(value);
        } else if name.contains("stale_frame") {
            self.stale_frames = Some(value);
        } else if name.contains("cpu_utilization_worst") {
            self.cpu_util_worst_pct = Some(value);
        } else if name.contains("cpu_utilization") {
            self.cpu_util_avg_pct = Some(value);
        } else if name.contains("gpu_utilization") {
            self.gpu_util_pct = Some(value);
        }
    }
}

fn format_metric(value: Option<f64>) -> String {
    match value {
        Some(value) => format!("{value:.3}"),
        None => "n/a".to_owned(),
    }
}

fn metric_counter_delta(previous: Option<f64>, current: Option<f64>) -> Option<f64> {
    Some((current? - previous?).max(0.0))
}

fn enumerate_counters(
    instance: &xr::Instance,
    fp: &xr::raw::PerformanceMetricsMETA,
) -> Result<Vec<(xr::sys::Path, String)>, xr::sys::Result> {
    let raw_instance = instance.as_raw();
    let mut count = 0u32;
    unsafe {
        let result = (fp.enumerate_performance_metrics_counter_paths)(
            raw_instance,
            0,
            &mut count,
            ptr::null_mut(),
        );
        if result.into_raw() < 0 {
            return Err(result);
        }
    }
    let mut paths = vec![xr::sys::Path::NULL; count as usize];
    unsafe {
        let result = (fp.enumerate_performance_metrics_counter_paths)(
            raw_instance,
            count,
            &mut count,
            paths.as_mut_ptr(),
        );
        if result.into_raw() < 0 {
            return Err(result);
        }
    }
    paths.truncate(count as usize);
    Ok(paths
        .into_iter()
        .map(|path| {
            let name = instance
                .path_to_string(path)
                .unwrap_or_else(|_| "<unnamed counter path>".to_owned());
            (path, name)
        })
        .collect())
}

fn set_metrics_enabled(
    fp: &xr::raw::PerformanceMetricsMETA,
    session: xr::sys::Session,
    enabled: bool,
) -> Result<(), xr::sys::Result> {
    let state = xr::sys::PerformanceMetricsStateMETA {
        ty: xr::sys::PerformanceMetricsStateMETA::TYPE,
        next: ptr::null(),
        enabled: enabled.into(),
    };
    let result = unsafe { (fp.set_performance_metrics_state)(session, &state) };
    if result.into_raw() < 0 {
        Err(result)
    } else {
        Ok(())
    }
}

fn query_counter(
    fp: &xr::raw::PerformanceMetricsMETA,
    session: xr::sys::Session,
    path: xr::sys::Path,
) -> Option<xr::sys::PerformanceMetricsCounterMETA> {
    let mut counter = xr::sys::PerformanceMetricsCounterMETA {
        ty: xr::sys::PerformanceMetricsCounterMETA::TYPE,
        next: ptr::null_mut(),
        counter_flags: xr::sys::PerformanceMetricsCounterFlagsMETA::EMPTY,
        counter_unit: xr::sys::PerformanceMetricsCounterUnitMETA::GENERIC,
        uint_value: 0,
        float_value: 0.0,
    };
    let result = unsafe { (fp.query_performance_metrics_counter)(session, path, &mut counter) };
    if result.into_raw() < 0 {
        None
    } else {
        Some(counter)
    }
}

fn query_named_counter_scalar(
    fp: &xr::raw::PerformanceMetricsMETA,
    session: xr::sys::Session,
    counters: &[(xr::sys::Path, String)],
    needle: &str,
) -> Option<f64> {
    counters
        .iter()
        .find(|(_, name)| name.contains(needle))
        .and_then(|(path, _)| query_counter(fp, session, *path))
        .and_then(|counter| counter_scalar(&counter))
}

fn counter_scalar(counter: &xr::sys::PerformanceMetricsCounterMETA) -> Option<f64> {
    let flags = counter.counter_flags;
    if flags.contains(xr::sys::PerformanceMetricsCounterFlagsMETA::FLOAT_VALUE_VALID) {
        Some(counter.float_value as f64)
    } else if flags.contains(xr::sys::PerformanceMetricsCounterFlagsMETA::UINT_VALUE_VALID) {
        Some(counter.uint_value as f64)
    } else {
        None
    }
}

fn unit_label(unit: xr::sys::PerformanceMetricsCounterUnitMETA) -> &'static str {
    use xr::sys::PerformanceMetricsCounterUnitMETA as Unit;
    if unit == Unit::PERCENTAGE {
        "%"
    } else if unit == Unit::MILLISECONDS {
        "ms"
    } else if unit == Unit::BYTES {
        "bytes"
    } else if unit == Unit::HERTZ {
        "Hz"
    } else {
        "generic"
    }
}

#[cfg(test)]
mod tests {
    use super::metric_counter_delta;

    #[test]
    fn dropped_frame_delta_requires_two_samples() {
        assert_eq!(metric_counter_delta(None, Some(7.0)), None);
        assert_eq!(metric_counter_delta(Some(7.0), None), None);
    }

    #[test]
    fn dropped_frame_delta_is_window_difference() {
        assert_eq!(metric_counter_delta(Some(7.0), Some(10.0)), Some(3.0));
    }

    #[test]
    fn dropped_frame_delta_saturates_after_counter_reset() {
        assert_eq!(metric_counter_delta(Some(10.0), Some(7.0)), Some(0.0));
    }
}
