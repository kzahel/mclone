#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const resultDir = path.resolve(process.argv[2] ?? "");
const manifestPath = path.join(resultDir, "matrix.tsv");
if (!process.argv[2] || !fs.existsSync(manifestPath)) {
  console.error("usage: summarize-perf-matrix.mjs RESULT_DIR");
  process.exit(2);
}

function parseTsv(text) {
  const lines = text.trim().split(/\r?\n/);
  const headers = lines.shift().split("\t");
  return lines.filter(Boolean).map((line) => {
    const values = line.split("\t");
    return Object.fromEntries(headers.map((header, index) => [header, values[index] ?? ""]));
  });
}

function numbers(values) {
  return values.filter((value) => Number.isFinite(value)).sort((a, b) => a - b);
}

function percentile(values, quantile) {
  const sorted = numbers(values);
  if (sorted.length === 0) return null;
  const index = Math.min(
    sorted.length - 1,
    Math.max(0, Math.ceil((sorted.length - 1) * quantile)),
  );
  return sorted[index];
}

function average(values) {
  const finite = numbers(values);
  return finite.length === 0
    ? null
    : finite.reduce((sum, value) => sum + value, 0) / finite.length;
}

function maximum(values) {
  const finite = numbers(values);
  return finite.length === 0 ? null : finite[finite.length - 1];
}

function delta(values) {
  const finite = values.filter((value) => Number.isFinite(value));
  return finite.length < 2 ? null : Math.max(...finite) - Math.min(...finite);
}

function workValues(work, key) {
  const components = key.split(".");
  return work
    .map((sample) => components.reduce((value, component) => value?.[component], sample))
    .map(Number)
    .filter(Number.isFinite);
}

function summarizeFrameSubset(samples) {
  if (samples.length === 0) return null;
  const wallMs = samples.map((sample) => Number(sample.frame_wall_ms));
  const totalWallMs = wallMs.reduce((sum, value) => sum + value, 0);
  return {
    frames: samples.length,
    average_fps: totalWallMs > 0 ? (samples.length * 1000) / totalWallMs : null,
    frame_wall_average_ms: average(wallMs),
    surface_encode_average_ms: average(
      samples.map((sample) => Number(sample.surface_encode_ms)),
    ),
    rebuilt_sections: samples.reduce(
      (sum, sample) => sum + Number(sample.work?.render_stream?.rebuilt_sections ?? 0),
      0,
    ),
  };
}

function summarizeSystemSamples(filePath, measurementSeconds) {
  if (!fs.existsSync(filePath)) return null;
  const lines = fs.readFileSync(filePath, "utf8").trim().split(/\r?\n/);
  const metadata = lines.shift()?.match(/clk_tck=(\d+)\s+cpu_count=(\d+)/);
  let samples = parseTsv(lines.join("\n")).map((sample) =>
    Object.fromEntries(
      Object.entries(sample).map(([key, value]) => [key, value === "" ? null : Number(value)]),
    ),
  );
  const finalTimestamp = samples.at(-1)?.unix_seconds;
  if (Number.isFinite(finalTimestamp) && Number.isFinite(measurementSeconds)) {
    const measurementStart = finalTimestamp - measurementSeconds;
    const measurementSamples = samples.filter(
      (sample) => sample.unix_seconds >= measurementStart,
    );
    if (measurementSamples.length >= 2) samples = measurementSamples;
  }
  if (samples.length === 0) return null;
  const first = samples[0];
  const last = samples[samples.length - 1];
  const elapsedSeconds = last.unix_seconds - first.unix_seconds;
  const clockTicks = Number(metadata?.[1] ?? 100);
  const cpuCount = Number(metadata?.[2] ?? 1);
  const processTickDelta = last.process_ticks - first.process_ticks;
  const systemTickDelta = last.system_ticks - first.system_ticks;
  const gpuTempMaxMillic = maximum(samples.map((sample) => sample.gpu_temp_millic));
  return {
    sample_count: samples.length,
    elapsed_seconds: elapsedSeconds,
    cpu_count: cpuCount,
    average_cpu_cores:
      elapsedSeconds > 0 ? processTickDelta / clockTicks / elapsedSeconds : null,
    average_cpu_total_percent:
      systemTickDelta > 0 ? (processTickDelta / systemTickDelta) * 100 : null,
    gpu_busy_average_percent: average(samples.map((sample) => sample.gpu_busy_percent)),
    gpu_busy_p95_percent: percentile(
      samples.map((sample) => sample.gpu_busy_percent),
      0.95,
    ),
    gpu_busy_max_percent: maximum(samples.map((sample) => sample.gpu_busy_percent)),
    gpu_clock_average_mhz: average(samples.map((sample) => sample.gpu_clock_mhz)),
    gpu_temp_max_celsius:
      gpuTempMaxMillic === null ? null : gpuTempMaxMillic / 1000,
    rss_max_kib: maximum(samples.map((sample) => sample.rss_kib)),
    data_max_kib: maximum(samples.map((sample) => sample.data_kib)),
    threads_max: maximum(samples.map((sample) => sample.threads)),
  };
}

function summarizeRow(manifest) {
  const reportPath = path.join(resultDir, manifest.report);
  const report = JSON.parse(fs.readFileSync(reportPath, "utf8"));
  const work = report.samples.map((sample) => sample.work).filter(Boolean);
  const measurementSeconds =
    report.samples.reduce((sum, sample) => sum + sample.frame_wall_ms, 0) / 1000;
  const featureCumulative = workValues(
    work,
    "scheduler.cumulative_feature_chunks_published",
  );
  const lightCumulative = workValues(
    work,
    "scheduler.cumulative_light_statuses_published",
  );
  const budgetSaturatedFrames = work.filter(
    (sample) =>
      sample.scheduler.feature_publish_budget_max_units > 0 &&
      sample.scheduler.feature_publish_spent_units >=
        sample.scheduler.feature_publish_budget_max_units,
  ).length;
  const firstCamera = report.samples.find((sample) => sample.camera_eye)?.camera_eye;
  const lastCamera = [...report.samples].reverse().find((sample) => sample.camera_eye)?.camera_eye;
  const cameraDistance =
    firstCamera && lastCamera
      ? Math.hypot(
          lastCamera[0] - firstCamera[0],
          lastCamera[1] - firstCamera[1],
          lastCamera[2] - firstCamera[2],
        )
      : 0;
  const quarterFrames = Math.max(1, Math.floor(report.samples.length / 4));
  const framesWithoutRebuild = report.samples.filter(
    (sample) => sample.work?.render_stream?.rebuilt_sections === 0,
  );
  const framesWithRebuild = report.samples.filter(
    (sample) => (sample.work?.render_stream?.rebuilt_sections ?? 0) > 0,
  );
  return {
    case: manifest.case,
    kind: manifest.kind,
    view: manifest.view,
    render_distance: Number(manifest.render_distance),
    velocity: manifest.velocity,
    adaptive_publication: manifest.adaptive_publication === "true",
    adaptive_admission: manifest.adaptive_admission === "true",
    world_scale_percent: Number(manifest.world_scale),
    freeze_fluids: manifest.freeze_fluids === "true",
    compile_capacity: manifest.compile_capacity,
    compile_workers: Number(manifest.compile_workers),
    compile_max_pending: Number(manifest.compile_max_pending),
    frames: report.frames,
    elapsed_wall_ms: report.elapsed_wall_ms,
    average_fps:
      report.elapsed_wall_ms > 0 ? (report.frames * 1000) / report.elapsed_wall_ms : null,
    camera_distance_blocks: cameraDistance,
    first_quarter: summarizeFrameSubset(report.samples.slice(0, quarterFrames)),
    last_quarter: summarizeFrameSubset(report.samples.slice(-quarterFrames)),
    frames_without_rebuild: summarizeFrameSubset(framesWithoutRebuild),
    frames_with_rebuild: summarizeFrameSubset(framesWithRebuild),
    frame_wall_p50_ms: report.frame_wall.p50_ms,
    frame_wall_p95_ms: report.frame_wall.p95_ms,
    frame_wall_p99_ms: report.frame_wall.p99_ms,
    frame_wall_max_ms: report.frame_wall.max_ms,
    over_2x_budget_frames: report.over_2x_budget_frames,
    over_4x_budget_frames: report.over_4x_budget_frames,
    runtime_poll_p95_ms: report.runtime_poll.p95_ms,
    render_p95_ms: report.render.p95_ms,
    surface_acquire_p95_ms: report.surface_acquire.p95_ms,
    surface_encode_p95_ms: report.surface_encode.p95_ms,
    gpu_total_p50_ms: report.gpu_total?.p50_ms ?? null,
    gpu_total_p95_ms: report.gpu_total?.p95_ms ?? null,
    gpu_terrain_p50_ms: report.gpu_terrain?.p50_ms ?? null,
    gpu_terrain_p95_ms: report.gpu_terrain?.p95_ms ?? null,
    pending_render_count_p95_ms: percentile(
      workValues(work, "render_stream.pending_render_count_ms"),
      0.95,
    ),
    traversal_ready_p95_ms: percentile(
      workValues(work, "render_stream.traversal_ready_sections_ms"),
      0.95,
    ),
    terrain_records_p95_ms: percentile(
      workValues(work, "render_timing.terrain_records_ms"),
      0.95,
    ),
    terrain_cull_p95_ms: percentile(
      workValues(work, "render_timing.terrain_cull_ms"),
      0.95,
    ),
    terrain_cull_cache_lookups: workValues(
      work,
      "render_timing.terrain_cull_cache_lookups",
    ).reduce((sum, value) => sum + value, 0),
    terrain_cull_cache_hits: workValues(
      work,
      "render_timing.terrain_cull_cache_hits",
    ).reduce((sum, value) => sum + value, 0),
    terrain_encode_p95_ms: percentile(
      workValues(work, "render_timing.terrain_encode_ms"),
      0.95,
    ),
    terrain_direct_draw_calls_average: average(
      workValues(work, "render_timing.terrain_direct_draw_calls"),
    ),
    terrain_multi_draw_calls_average: average(
      workValues(work, "render_timing.terrain_multi_draw_calls"),
    ),
    terrain_indirect_draw_count_average: average(
      workValues(work, "render_timing.terrain_indirect_draw_count"),
    ),
    terrain_arena_vertex_used_bytes_average: average(
      workValues(work, "render_timing.terrain_arena_vertex_used_bytes"),
    ),
    terrain_arena_vertex_capacity_bytes_max: maximum(
      workValues(work, "render_timing.terrain_arena_vertex_capacity_bytes"),
    ),
    terrain_arena_index_used_bytes_average: average(
      workValues(work, "render_timing.terrain_arena_index_used_bytes"),
    ),
    terrain_arena_index_capacity_bytes_max: maximum(
      workValues(work, "render_timing.terrain_arena_index_capacity_bytes"),
    ),
    scheduler_tick_p95_ms: percentile(workValues(work, "scheduler.tick_ms"), 0.95),
    scheduler_tick_max_ms: maximum(workValues(work, "scheduler.tick_ms")),
    scheduler_reconcile_holders_p95_ms: percentile(
      workValues(work, "scheduler.reconcile_holders_ms"),
      0.95,
    ),
    server_total_p95_ms: percentile(workValues(work, "server.reported_total_ms"), 0.95),
    drawn_sections_average: average(workValues(work, "render.drawn_section_count")),
    drawn_sections_p95: percentile(workValues(work, "render.drawn_section_count"), 0.95),
    drawn_indices_average: average(workValues(work, "render.drawn_index_count")),
    generation_queue_max: maximum(
      workValues(work, "scheduler.pending_worldgen_publication_chunks"),
    ),
    generation_jobs_max: maximum(workValues(work, "server.pending_jobs")),
    publication_backlog_max: maximum(workValues(work, "server.pending_publications")),
    generation_mailbox_max: maximum(
      workValues(work, "scheduler.worldgen_mailbox_pending_jobs"),
    ),
    light_publication_queue_max: maximum(
      workValues(work, "scheduler.pending_light_publications"),
    ),
    server_update_queue_bytes_max: maximum(workValues(work, "server.update_queue_bytes")),
    server_update_oldest_age_max_ms: maximum(
      workValues(work, "server.update_oldest_applied_age_ms"),
    ),
    render_chunk_queue_max: maximum(
      workValues(work, "render_stream.pending_render_chunks_after"),
    ),
    render_compile_queue_max: maximum(
      workValues(work, "render_stream.pending_compile_jobs_after"),
    ),
    feature_chunks_published: delta(featureCumulative),
    light_statuses_published: delta(lightCumulative),
    feature_publish_budget_max_average: average(
      workValues(work, "scheduler.feature_publish_budget_max_units"),
    ),
    feature_publish_spent_average: average(
      workValues(work, "scheduler.feature_publish_spent_units"),
    ),
    light_publish_budget_max_average: average(
      workValues(work, "scheduler.light_publish_budget_max_units"),
    ),
    light_publish_spent_average: average(
      workValues(work, "scheduler.light_publish_spent_units"),
    ),
    budget_saturated_frame_samples: budgetSaturatedFrames,
    deadline_skipped_compile_requests: workValues(
      work,
      "render_stream.deadline_skipped_compile_requests",
    ).reduce((sum, value) => sum + value, 0),
    rebuilt_sections: workValues(work, "render_stream.rebuilt_sections").reduce(
      (sum, value) => sum + value,
      0,
    ),
    uploaded_sections: workValues(work, "render_stream.uploaded_sections").reduce(
      (sum, value) => sum + value,
      0,
    ),
    submitted_compile_sections: workValues(
      work,
      "render_stream.submitted_compile_sections",
    ).reduce((sum, value) => sum + value, 0),
    completed_compile_sections: workValues(
      work,
      "render_stream.completed_compile_sections",
    ).reduce((sum, value) => sum + value, 0),
    stale_compile_sections: workValues(
      work,
      "render_stream.stale_compile_sections",
    ).reduce((sum, value) => sum + value, 0),
    fluid_due_ticks: workValues(work, "render_stream.fluid_due_ticks").reduce(
      (sum, value) => sum + value,
      0,
    ),
    fluid_executed_ticks: workValues(work, "render_stream.fluid_executed_ticks").reduce(
      (sum, value) => sum + value,
      0,
    ),
    fluid_mutated_blocks: workValues(work, "render_stream.fluid_mutated_blocks").reduce(
      (sum, value) => sum + value,
      0,
    ),
    compile_worker_count_max: maximum(
      workValues(work, "render_stream.compile_worker_count"),
    ),
    completed_compile_tasks: delta(
      workValues(work, "render_stream.completed_compile_tasks"),
    ),
    compile_worker_busy_ms: delta(
      workValues(work, "render_stream.total_compile_worker_busy_ms"),
    ),
    compile_worker_task_max_ms: maximum(
      workValues(work, "render_stream.max_compile_worker_task_ms"),
    ),
    update_pump_stall_count_max: maximum(
      workValues(work, "server.update_pump_stall_count"),
    ),
    final_runtime: report.final_runtime,
    final_scheduler: report.final_scheduler,
    system: summarizeSystemSamples(
      path.join(resultDir, `${manifest.case}.system.tsv`),
      measurementSeconds,
    ),
  };
}

function fixed(value, digits = 2) {
  return Number.isFinite(value) ? value.toFixed(digits) : "n/a";
}

const manifest = parseTsv(fs.readFileSync(manifestPath, "utf8"));
const rows = manifest.map(summarizeRow);
const summary = {
  schema: 1,
  benchmark: "steam_deck_stationary_traversal_matrix",
  result_directory: resultDir,
  rows,
};
fs.writeFileSync(
  path.join(resultDir, "matrix-summary.json"),
  `${JSON.stringify(summary, null, 2)}\n`,
);

const markdown = [
  "| Case | RD | Motion/view | FPS | p95 / p99 ms | Surface encode p95 | Terrain cull / encode p95 | GPU terrain p50 / p95 | Draw sections | Rebuilt | CPU cores | GPU avg |",
  "|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
  ...rows.map((row) => {
    const motion = `${row.kind}/${row.view}`;
    return `| ${row.case} | ${row.render_distance} | ${motion} | ${fixed(row.average_fps, 1)} | ${fixed(row.frame_wall_p95_ms)} / ${fixed(row.frame_wall_p99_ms)} | ${fixed(row.surface_encode_p95_ms)} | ${fixed(row.terrain_cull_p95_ms)} / ${fixed(row.terrain_encode_p95_ms)} | ${fixed(row.gpu_terrain_p50_ms)} / ${fixed(row.gpu_terrain_p95_ms)} | ${fixed(row.drawn_sections_average, 0)} | ${fixed(row.rebuilt_sections, 0)} | ${fixed(row.system?.average_cpu_cores)} | ${fixed(row.system?.gpu_busy_average_percent, 1)}% |`;
  }),
  "",
  "| Attribution case | Scale | Fluids | Pending count p95 | Ready stamp p95 | Records p95 | Cull cache hits | Reconcile p95 | Compile submit / done / stale | GPU total p50 / p95 | Fluid due / run / mutations |",
  "|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|",
  ...rows.map(
    (row) =>
      `| ${row.case} | ${fixed(row.world_scale_percent, 0)}% | ${row.freeze_fluids ? "frozen" : "live"} | ${fixed(row.pending_render_count_p95_ms)} | ${fixed(row.traversal_ready_p95_ms)} | ${fixed(row.terrain_records_p95_ms)} | ${fixed(row.terrain_cull_cache_hits, 0)} / ${fixed(row.terrain_cull_cache_lookups, 0)} | ${fixed(row.scheduler_reconcile_holders_p95_ms)} | ${fixed(row.submitted_compile_sections, 0)} / ${fixed(row.completed_compile_sections, 0)} / ${fixed(row.stale_compile_sections, 0)} | ${fixed(row.gpu_total_p50_ms)} / ${fixed(row.gpu_total_p95_ms)} | ${fixed(row.fluid_due_ticks, 0)} / ${fixed(row.fluid_executed_ticks, 0)} / ${fixed(row.fluid_mutated_blocks, 0)} |`,
  ),
  "",
  "| Submission case | Direct / multi calls avg | Indirect draws avg | Vertex used / cap MiB | Index used / cap MiB |",
  "|---|---:|---:|---:|---:|",
  ...rows.map(
    (row) =>
      `| ${row.case} | ${fixed(row.terrain_direct_draw_calls_average, 1)} / ${fixed(row.terrain_multi_draw_calls_average, 1)} | ${fixed(row.terrain_indirect_draw_count_average, 1)} | ${fixed(row.terrain_arena_vertex_used_bytes_average / 1048576, 1)} / ${fixed(row.terrain_arena_vertex_capacity_bytes_max / 1048576, 1)} | ${fixed(row.terrain_arena_index_used_bytes_average / 1048576, 1)} / ${fixed(row.terrain_arena_index_capacity_bytes_max / 1048576, 1)} |`,
  ),
  "",
  "| Worker case | Capacity | Workers | Busy ms | Completed | Task max ms | Compile q max |",
  "|---|---|---:|---:|---:|---:|---:|",
  ...rows.map(
    (row) =>
      `| ${row.case} | ${row.compile_capacity} | ${fixed(row.compile_worker_count_max ?? row.compile_workers, 0)} | ${fixed(row.compile_worker_busy_ms)} | ${fixed(row.completed_compile_tasks, 0)} | ${fixed(row.compile_worker_task_max_ms)} | ${fixed(row.render_compile_queue_max, 0)} |`,
  ),
  "",
  "| Traversal case | Travel | Jobs / pubs max | Gen queue / mailbox max | Feature / light published | Budget avg max/spent | Saturated samples | Update max KiB / age ms | Render / compile q max | Rebuilt / uploaded |",
  "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
  ...rows.filter((row) => row.kind === "traversal").map(
    (row) =>
      `| ${row.case} | ${fixed(row.camera_distance_blocks, 1)} | ${fixed(row.generation_jobs_max, 0)} / ${fixed(row.publication_backlog_max, 0)} | ${fixed(row.generation_queue_max, 0)} / ${fixed(row.generation_mailbox_max, 0)} | ${fixed(row.feature_chunks_published, 0)} / ${fixed(row.light_statuses_published, 0)} | ${fixed(row.feature_publish_budget_max_average, 1)}/${fixed(row.feature_publish_spent_average, 1)} | ${fixed(row.budget_saturated_frame_samples, 0)} | ${fixed(Number.isFinite(row.server_update_queue_bytes_max) ? row.server_update_queue_bytes_max / 1024 : null, 1)} / ${fixed(row.server_update_oldest_age_max_ms, 1)} | ${fixed(row.render_chunk_queue_max, 0)} / ${fixed(row.render_compile_queue_max, 0)} | ${fixed(row.rebuilt_sections, 0)} / ${fixed(row.uploaded_sections, 0)} |`,
  ),
  "",
  "| Stationary case | First / last quarter FPS | No-rebuild frames @ FPS / encode ms | Rebuild frames @ FPS / encode ms | Rebuilt sections | Scheduled fluids at end |",
  "|---|---:|---:|---:|---:|---:|",
  ...rows.filter((row) => row.kind === "stationary").map(
    (row) =>
      `| ${row.case} | ${fixed(row.first_quarter?.average_fps, 1)} / ${fixed(row.last_quarter?.average_fps, 1)} | ${fixed(row.frames_without_rebuild?.frames, 0)} @ ${fixed(row.frames_without_rebuild?.average_fps, 1)} / ${fixed(row.frames_without_rebuild?.surface_encode_average_ms, 2)} | ${fixed(row.frames_with_rebuild?.frames, 0)} @ ${fixed(row.frames_with_rebuild?.average_fps, 1)} / ${fixed(row.frames_with_rebuild?.surface_encode_average_ms, 2)} | ${fixed(row.rebuilt_sections, 0)} | ${fixed(row.final_runtime?.scheduled_fluid_ticks, 0)} |`,
  ),
  "",
];
const markdownText = `${markdown.join("\n")}\n`;
fs.writeFileSync(path.join(resultDir, "matrix-summary.md"), markdownText);
process.stdout.write(markdownText);
