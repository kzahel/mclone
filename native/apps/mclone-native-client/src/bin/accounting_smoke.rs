use std::hint::black_box;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use mclone_diagnostics::{
    CriticalPathLabel, FrameAccountingConfig, FrameAccumulator, FrameObservation,
    FramePipelineReport, GpuTimestampPanelReport, QueuePanelReport, StageId, StageSpan, clock,
};
use mclone_render::gpu_timestamps::run_gpu_timestamp_calibration;
use mclone_render::headless::create_headless_device;
use serde_json::json;

// Windows GetThreadTimes commonly advances in ~15.625 ms steps on this lane.
#[cfg(windows)]
const DEFAULT_SPIN_MS: f64 = 200.0;
#[cfg(not(windows))]
const DEFAULT_SPIN_MS: f64 = 6.0;
const TARGET_PERIOD_MS: f64 = 16.667;
const TOLERANCE_FRACTION: f64 = 0.10;
const MIN_TOLERANCE_MS: f64 = 0.3;
const MAX_SPIN_MS: f64 = 250.0;

#[derive(Clone, Copy, Debug)]
struct Options {
    spin_ms: f64,
    break_attribution: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            spin_ms: DEFAULT_SPIN_MS,
            break_attribution: false,
        }
    }
}

fn main() -> Result<()> {
    let options = parse_options(std::env::args().skip(1))?;
    let tolerance_ms = attribution_tolerance_ms(options.spin_ms);
    let spin = busy_spin(options.spin_ms);
    let stage = if options.break_attribution {
        StageId::CpuMeshCompile
    } else {
        StageId::DrawEncode
    };

    let mut accumulator = FrameAccumulator::new(
        FrameAccountingConfig::from_target_period_ms(TARGET_PERIOD_MS)
            .with_conservation_tolerance_ms(tolerance_ms),
    );
    accumulator.record_frame(
        FrameObservation::new(1, spin.wall_ms)
            .with_thread_cpu_ms(spin.thread_cpu_ms)
            .with_stage_span(
                StageSpan::new(stage, spin.wall_ms).with_thread_cpu_ms(spin.thread_cpu_ms),
            ),
    );
    let summary = accumulator.summary_report();
    let Some(draw_encode) = summary
        .latest_stage_spans
        .iter()
        .find(|span| span.stage == StageId::DrawEncode)
    else {
        bail!("accounting smoke did not attribute the spin to draw-encode");
    };
    if draw_encode.label != CriticalPathLabel::CurrentFrameCritical {
        bail!(
            "accounting smoke draw-encode label was {:?}, expected {:?}",
            draw_encode.label,
            CriticalPathLabel::CurrentFrameCritical
        );
    }
    let attributed_ms = draw_encode.thread_cpu_ms.unwrap_or(draw_encode.elapsed_ms);
    let attribution_error_ms = (attributed_ms - options.spin_ms).abs();
    if attribution_error_ms > tolerance_ms {
        bail!(
            "accounting smoke attributed {:.3} ms, expected {:.3} ms within {:.3} ms",
            attributed_ms,
            options.spin_ms,
            tolerance_ms
        );
    }
    if !summary.conservation_violations.is_empty() {
        bail!(
            "accounting smoke produced conservation violations: {:?}",
            summary.conservation_violations
        );
    }

    let (gpu_calibration, gpu_timestamp_panel) = run_gpu_calibration_smoke()?;
    let pipeline = FramePipelineReport::new(summary, QueuePanelReport::new(Vec::new()))
        .with_gpu_timestamp_panel(gpu_timestamp_panel);
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "benchmark": "native_accounting_smoke",
            "target_spin_ms": options.spin_ms,
            "tolerance_ms": tolerance_ms,
            "thread_cpu_supported": spin.thread_cpu_ms.is_some(),
            "wall_spin_ms": spin.wall_ms,
            "thread_cpu_spin_ms": spin.thread_cpu_ms,
            "attributed_ms": attributed_ms,
            "attribution_error_ms": attribution_error_ms,
            "break_attribution": options.break_attribution,
            "gpu_timestamp_calibration": gpu_calibration,
            "frame_pipeline_accounting": pipeline,
        }))
        .context("serialize accounting smoke report")?
    );
    Ok(())
}

fn run_gpu_calibration_smoke() -> Result<(serde_json::Value, GpuTimestampPanelReport)> {
    let (device, queue) = create_headless_device().context("create headless GPU device")?;
    let renderdoc_capture = std::env::var_os("MCLONE_RENDERDOC_CAPTURE").is_some();
    if renderdoc_capture {
        // SAFETY: This only asks an injected graphics debugger to begin a frame capture
        // around the validation-only calibration pass. Normal smoke runs leave it off.
        unsafe {
            device.start_graphics_debugger_capture();
        }
    }
    let report = run_gpu_timestamp_calibration(&device, &queue, 2048);
    if renderdoc_capture {
        // SAFETY: Pairs with the optional capture start above.
        unsafe {
            device.stop_graphics_debugger_capture();
        }
    }
    let report = report.context("run GPU timestamp calibration")?;
    let panel = report.panel.clone();
    Ok((
        json!({
            "supported": report.supported,
            "timestamp_period_ns": report.timestamp_period_ns,
            "light_pass_ms": report.light_pass_ms,
            "light_pass_valid": report.light_pass_valid,
            "heavy_pass_ms": report.heavy_pass_ms,
            "heavy_pass_valid": report.heavy_pass_valid,
            "heavy_quad_count": report.heavy_quad_count,
            "scale_ratio": report.scale_ratio,
            "panel": report.panel,
        }),
        panel,
    ))
}

fn parse_options(args: impl IntoIterator<Item = String>) -> Result<Options> {
    let mut options = Options::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--spin-ms" => {
                let value = args.next().context("--spin-ms requires milliseconds")?;
                options.spin_ms = value
                    .parse::<f64>()
                    .with_context(|| format!("--spin-ms requires a number, got `{value}`"))?;
                if !options.spin_ms.is_finite()
                    || options.spin_ms <= 0.0
                    || options.spin_ms > MAX_SPIN_MS
                {
                    bail!("--spin-ms must be > 0 and <= {MAX_SPIN_MS}");
                }
            }
            "--break-attribution" => {
                options.break_attribution = true;
            }
            "--help" | "-h" => {
                println!("accounting_smoke [--spin-ms {DEFAULT_SPIN_MS}] [--break-attribution]");
                std::process::exit(0);
            }
            _ => bail!("unknown argument `{arg}`"),
        }
    }
    Ok(options)
}

#[derive(Clone, Copy, Debug)]
struct SpinResult {
    wall_ms: f64,
    thread_cpu_ms: Option<f64>,
}

fn busy_spin(target_ms: f64) -> SpinResult {
    let wall_start = Instant::now();
    let thread_cpu_start = clock::thread_cpu_time_ms();
    let wall_cap_ms = (target_ms * 10.0).max(target_ms + 1.0);
    let mut counter = 0_u64;
    loop {
        counter = counter.wrapping_add(1);
        black_box(counter);
        let wall_ms = wall_start.elapsed().as_secs_f64() * 1000.0;
        let thread_cpu_elapsed_ms = thread_cpu_start
            .and_then(|start| clock::thread_cpu_time_ms().map(|now| (now - start).max(0.0)));
        if thread_cpu_elapsed_ms.is_some_and(|elapsed_ms| elapsed_ms >= target_ms)
            || (thread_cpu_elapsed_ms.is_none() && wall_ms >= target_ms)
            || wall_ms >= wall_cap_ms
        {
            break;
        }
    }
    let wall_ms = wall_start.elapsed().as_secs_f64() * 1000.0;
    let thread_cpu_ms = thread_cpu_start
        .and_then(|start| clock::thread_cpu_time_ms().map(|now| (now - start).max(0.0)));
    SpinResult {
        wall_ms,
        thread_cpu_ms,
    }
}

fn attribution_tolerance_ms(target_ms: f64) -> f64 {
    (target_ms * TOLERANCE_FRACTION).max(MIN_TOLERANCE_MS)
}
