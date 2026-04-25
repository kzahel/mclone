package net.minecraft.world.level.chunk;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.mojang.datafixers.util.Either;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Registry;
import net.minecraft.server.level.ChunkHolder;
import net.minecraft.server.level.WorldGenRegion;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.block.state.BlockState;

final class McloneSchedulerTraceRecorder {
   private static final Gson GSON = new GsonBuilder().setPrettyPrinting().disableHtmlEscaping().create();
   private static final boolean ENABLED = Boolean.getBoolean("mclone.schedulerTrace.enabled");
   private static final boolean HALT_ON_COMPLETE = Boolean.parseBoolean(System.getProperty("mclone.schedulerTrace.haltOnComplete", "false"));
   private static final String OUTPUT = System.getProperty("mclone.schedulerTrace.output", "");
   private static final String SCENARIO = System.getProperty("mclone.schedulerTrace.scenario", "spawn_bootstrap");
   private static final String SEED = System.getProperty("mclone.schedulerTrace.seed", "unknown");
   private static final int TARGET_CHUNK_X = parseIntProperty("mclone.schedulerTrace.chunkX", 0);
   private static final int TARGET_CHUNK_Z = parseIntProperty("mclone.schedulerTrace.chunkZ", 0);
   private static final int TARGET_RADIUS = parseIntProperty("mclone.schedulerTrace.radius", 1);
   private static final int RECORD_RADIUS = parseIntProperty("mclone.schedulerTrace.recordRadius", TARGET_RADIUS);
   private static final String STOP_STATUS = System.getProperty("mclone.schedulerTrace.stopStatus", "features").toLowerCase(Locale.ROOT);
   private static final List<ProbeBlock> PROBE_BLOCKS = parseProbeBlocks(System.getProperty("mclone.schedulerTrace.probeBlocks", ""));
   private static final List<Map<String, Object>> EVENTS = new ArrayList<>();
   private static final Set<String> TARGET_STOP_COMPLETIONS = new LinkedHashSet<>();
   private static long sequence;
   private static boolean written;

   private McloneSchedulerTraceRecorder() {
   }

   static void recordDependencyReady(ChunkStatus status, ChunkAccess center, List<ChunkAccess> chunks) {
      record("dependency_ready", status, center, chunks, null);
   }

   static void recordTaskStart(ChunkStatus status, ChunkAccess center, List<ChunkAccess> chunks) {
      record("task_start", status, center, chunks, null);
   }

   static CompletableFuture<Either<ChunkAccess, ChunkHolder.ChunkLoadingFailure>> watchTaskComplete(
      ChunkStatus status,
      ChunkAccess center,
      List<ChunkAccess> chunks,
      CompletableFuture<Either<ChunkAccess, ChunkHolder.ChunkLoadingFailure>> future
   ) {
      if (!ENABLED) {
         return future;
      }

      return future.whenComplete((result, error) -> record(error == null ? "task_complete" : "task_failed", status, center, chunks, error));
   }

   static void recordTaskThrow(ChunkStatus status, ChunkAccess center, List<ChunkAccess> chunks, Throwable error) {
      record("task_failed", status, center, chunks, error);
   }

   static boolean hasProbeBlocks() {
      return !PROBE_BLOCKS.isEmpty();
   }

   static synchronized void recordFeatureProbe(
      WorldGenRegion region,
      int stepIndex,
      int featureIndex,
      String configuredFeature,
      String featureType,
      int randomCount
   ) {
      if (!ENABLED || written || PROBE_BLOCKS.isEmpty()) {
         return;
      }

      ChunkPos pos = region.getCenter();
      if (!isInRecordWindow(pos) && !isInTargetNeighborhood(pos)) {
         return;
      }

      Map<String, Object> event = new LinkedHashMap<>();
      event.put("sequence", ++sequence);
      event.put("phase", "feature_probe");
      event.put("status", "FEATURES");
      event.put("statusName", "features");
      event.put("chunkX", pos.x);
      event.put("chunkZ", pos.z);
      event.put("scenario", SCENARIO);
      event.put("seed", SEED);
      event.put("targetChunkX", TARGET_CHUNK_X);
      event.put("targetChunkZ", TARGET_CHUNK_Z);
      event.put("targetRadius", TARGET_RADIUS);
      event.put("stepIndex", stepIndex);
      event.put("featureIndex", featureIndex);
      event.put("configuredFeature", configuredFeature);
      event.put("featureType", featureType);
      event.put("randomCount", randomCount);
      event.put("thread", Thread.currentThread().getName());
      appendProbeBlocks(event, region.getChunk(TARGET_CHUNK_X, TARGET_CHUNK_Z, ChunkStatus.EMPTY, false));
      EVENTS.add(event);
   }

   private static int parseIntProperty(String name, int defaultValue) {
      String value = System.getProperty(name);
      if (value == null || value.isEmpty()) {
         return defaultValue;
      }

      return Integer.parseInt(value);
   }

   private static synchronized void record(String phase, ChunkStatus status, ChunkAccess center, List<ChunkAccess> chunks, Throwable error) {
      if (!ENABLED || written) {
         return;
      }

      ChunkPos pos = center.getPos();
      if (!isInRecordWindow(pos) && !isInTargetNeighborhood(pos)) {
         return;
      }

      Map<String, Object> event = new LinkedHashMap<>();
      event.put("sequence", ++sequence);
      event.put("phase", phase);
      event.put("status", status.getName().toUpperCase(Locale.ROOT));
      event.put("statusName", status.getName());
      event.put("chunkX", pos.x);
      event.put("chunkZ", pos.z);
      event.put("scenario", SCENARIO);
      event.put("seed", SEED);
      event.put("targetChunkX", TARGET_CHUNK_X);
      event.put("targetChunkZ", TARGET_CHUNK_Z);
      event.put("targetRadius", TARGET_RADIUS);
      event.put("statusDependencyRange", status.getRange());
      event.put("dependencyChunkCount", chunks.size());
      appendDependencyBounds(event, chunks);
      event.put("thread", Thread.currentThread().getName());
      if (error != null) {
         event.put("error", error.getClass().getName() + ": " + error.getMessage());
      }
      appendProbeBlocks(event, chunks);
      EVENTS.add(event);

      if ("task_complete".equals(phase) && status.getName().equals(STOP_STATUS) && isInTargetNeighborhood(pos)) {
         TARGET_STOP_COMPLETIONS.add(chunkKey(pos));
         if (TARGET_STOP_COMPLETIONS.size() == expectedTargetChunkCount()) {
            writeTrace();
         }
      }
   }

   private static void appendDependencyBounds(Map<String, Object> event, List<ChunkAccess> chunks) {
      if (chunks.isEmpty()) {
         return;
      }

      int minX = Integer.MAX_VALUE;
      int minZ = Integer.MAX_VALUE;
      int maxX = Integer.MIN_VALUE;
      int maxZ = Integer.MIN_VALUE;
      for (ChunkAccess chunk : chunks) {
         ChunkPos pos = chunk.getPos();
         minX = Math.min(minX, pos.x);
         minZ = Math.min(minZ, pos.z);
         maxX = Math.max(maxX, pos.x);
         maxZ = Math.max(maxZ, pos.z);
      }

      event.put("dependencyMinChunkX", minX);
      event.put("dependencyMinChunkZ", minZ);
      event.put("dependencyMaxChunkX", maxX);
      event.put("dependencyMaxChunkZ", maxZ);
   }

   private static void appendProbeBlocks(Map<String, Object> event, List<ChunkAccess> chunks) {
      if (PROBE_BLOCKS.isEmpty()) {
         return;
      }

      ChunkAccess target = null;
      for (ChunkAccess chunk : chunks) {
         ChunkPos pos = chunk.getPos();
         if (pos.x == TARGET_CHUNK_X && pos.z == TARGET_CHUNK_Z) {
            target = chunk;
            break;
         }
      }
      if (target == null) {
         return;
      }

      appendProbeBlocks(event, target);
   }

   private static void appendProbeBlocks(Map<String, Object> event, ChunkAccess target) {
      if (target == null) {
         return;
      }

      List<Map<String, Object>> probes = new ArrayList<>();
      for (ProbeBlock probe : PROBE_BLOCKS) {
         BlockPos pos = new BlockPos(
            (TARGET_CHUNK_X << 4) + probe.localX,
            probe.y,
            (TARGET_CHUNK_Z << 4) + probe.localZ
         );
         BlockState state = target.getBlockState(pos);
         Map<String, Object> row = new LinkedHashMap<>();
         row.put("localX", probe.localX);
         row.put("y", probe.y);
         row.put("localZ", probe.localZ);
         row.put("block", Registry.BLOCK.getKey(state.getBlock()).toString());
         probes.add(row);
      }
      event.put("probeBlocks", probes);
   }

   private static List<ProbeBlock> parseProbeBlocks(String raw) {
      if (raw == null || raw.isBlank()) {
         return Collections.emptyList();
      }

      List<ProbeBlock> blocks = new ArrayList<>();
      for (String entry : raw.split(";")) {
         String trimmed = entry.trim();
         if (trimmed.isEmpty()) {
            continue;
         }

         String[] parts = trimmed.split(",");
         if (parts.length != 3) {
            throw new IllegalArgumentException("invalid probe block: " + entry);
         }
         blocks.add(new ProbeBlock(
            Integer.parseInt(parts[0].trim()),
            Integer.parseInt(parts[1].trim()),
            Integer.parseInt(parts[2].trim())
         ));
      }
      return blocks;
   }

   private static boolean isInRecordWindow(ChunkPos pos) {
      return Math.abs(pos.x - TARGET_CHUNK_X) <= RECORD_RADIUS && Math.abs(pos.z - TARGET_CHUNK_Z) <= RECORD_RADIUS;
   }

   private static boolean isInTargetNeighborhood(ChunkPos pos) {
      return Math.abs(pos.x - TARGET_CHUNK_X) <= TARGET_RADIUS && Math.abs(pos.z - TARGET_CHUNK_Z) <= TARGET_RADIUS;
   }

   private static int expectedTargetChunkCount() {
      int diameter = TARGET_RADIUS * 2 + 1;
      return diameter * diameter;
   }

   private static String chunkKey(ChunkPos pos) {
      return pos.x + "," + pos.z;
   }

   private static final class ProbeBlock {
      final int localX;
      final int y;
      final int localZ;

      ProbeBlock(int localX, int y, int localZ) {
         this.localX = localX;
         this.y = y;
         this.localZ = localZ;
      }
   }

   private static void writeTrace() {
      if (written) {
         return;
      }

      written = true;
      Map<String, Object> root = new LinkedHashMap<>();
      root.put("module", "scheduler-trace");
      root.put("traceKind", "instrumented-deobf-server");
      root.put("minecraftVersion", "1.17.1");
      root.put("seed", SEED);
      root.put("scenario", SCENARIO);
      root.put("targetChunkX", TARGET_CHUNK_X);
      root.put("targetChunkZ", TARGET_CHUNK_Z);
      root.put("targetRadius", TARGET_RADIUS);
      root.put("recordRadius", RECORD_RADIUS);
      root.put("stopStatus", STOP_STATUS.toUpperCase(Locale.ROOT));
      root.put("availableProcessors", Runtime.getRuntime().availableProcessors());
      root.put("identityHashCodeMode", System.getProperty("mclone.schedulerTrace.identityHashCodeMode", "default"));
      root.put("sourceHooks", new String[]{
         "ChunkStatus.generate:dependency_ready",
         "ChunkStatus.generate:task_start",
         "ChunkStatus.generate:task_complete"
      });
      root.put("featureCompletionOrder3x3", collectCompletionOrder("features"));
      root.put("fullCompletionOrder3x3", collectCompletionOrder("full"));
      root.put("events", new ArrayList<>(EVENTS));

      try {
         Files.createDirectories(Path.of(OUTPUT).getParent());
         Files.writeString(Path.of(OUTPUT), GSON.toJson(root) + "\n");
      } catch (IOException error) {
         throw new RuntimeException("failed to write scheduler trace to " + OUTPUT, error);
      }

      if (HALT_ON_COMPLETE) {
         Thread haltThread = new Thread(() -> {
            try {
               Thread.sleep(50L);
            } catch (InterruptedException ignored) {
            }
            Runtime.getRuntime().halt(0);
         }, "mclone-scheduler-trace-halt");
         haltThread.setDaemon(true);
         haltThread.start();
      }
   }

   private static List<Map<String, Object>> collectCompletionOrder(String statusName) {
      List<Map<String, Object>> order = new ArrayList<>();
      String status = statusName.toUpperCase(Locale.ROOT);
      for (Map<String, Object> event : EVENTS) {
         if (!"task_complete".equals(event.get("phase")) || !status.equals(event.get("status"))) {
            continue;
         }

         int chunkX = ((Number)event.get("chunkX")).intValue();
         int chunkZ = ((Number)event.get("chunkZ")).intValue();
         if (Math.abs(chunkX - TARGET_CHUNK_X) > TARGET_RADIUS || Math.abs(chunkZ - TARGET_CHUNK_Z) > TARGET_RADIUS) {
            continue;
         }

         Map<String, Object> entry = new LinkedHashMap<>();
         entry.put("sequence", event.get("sequence"));
         entry.put("chunkX", chunkX);
         entry.put("chunkZ", chunkZ);
         order.add(entry);
      }

      return order;
   }
}
