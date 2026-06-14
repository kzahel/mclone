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
import java.util.OptionalInt;
import java.util.Random;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Registry;
import net.minecraft.server.level.ChunkHolder;
import net.minecraft.server.level.WorldGenRegion;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.WorldgenRandom;

public final class McloneSchedulerTraceRecorder {
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
   private static final boolean PROBE_TARGET_TREE_BLOCKS = Boolean.parseBoolean(System.getProperty("mclone.schedulerTrace.probeTargetTreeBlocks", "false"));
   private static final boolean PROBE_TREE_CANDIDATES = Boolean.parseBoolean(System.getProperty("mclone.schedulerTrace.probeTreeCandidates", "false"));
   private static final int TREE_PROBE_CENTER_X = parseIntProperty("mclone.schedulerTrace.treeProbeCenterX", Integer.MIN_VALUE);
   private static final int TREE_PROBE_CENTER_Z = parseIntProperty("mclone.schedulerTrace.treeProbeCenterZ", Integer.MIN_VALUE);
   private static final int TREE_PROBE_STEP_INDEX = parseIntProperty("mclone.schedulerTrace.treeProbeStepIndex", -1);
   private static final int TREE_PROBE_FEATURE_INDEX = parseIntProperty("mclone.schedulerTrace.treeProbeFeatureIndex", -1);
   private static final List<Map<String, Object>> EVENTS = new ArrayList<>();
   private static final Set<String> TARGET_STOP_COMPLETIONS = new LinkedHashSet<>();
   private static final ThreadLocal<FeatureProbeContext> CURRENT_FEATURE = new ThreadLocal<>();
   private static final ThreadLocal<TreeCandidateProbe> CURRENT_TREE_CANDIDATE = new ThreadLocal<>();
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

   static boolean hasFeatureProbes() {
      return hasProbeBlocks() || PROBE_TARGET_TREE_BLOCKS || PROBE_TREE_CANDIDATES;
   }

   static void enterFeatureContext(
      WorldGenRegion region,
      int stepIndex,
      int featureIndex,
      String configuredFeature,
      String featureType
   ) {
      if (!ENABLED || written || !PROBE_TREE_CANDIDATES) {
         return;
      }

      CURRENT_FEATURE.set(new FeatureProbeContext(region.getCenter(), stepIndex, featureIndex, configuredFeature, featureType));
   }

   static void exitFeatureContext() {
      CURRENT_FEATURE.remove();
   }

   public static boolean beginTreeCandidate(
      String treeConfiguredFeature,
      String treeFeatureType,
      Random random,
      BlockPos origin
   ) {
      if (!ENABLED || written || !PROBE_TREE_CANDIDATES) {
         return false;
      }

      FeatureProbeContext context = CURRENT_FEATURE.get();
      if (context == null || !matchesTreeProbeFilter(context)) {
         return false;
      }

      Map<String, Object> event = new LinkedHashMap<>();
      event.put("phase", "tree_candidate");
      event.put("status", "FEATURES");
      event.put("statusName", "features");
      event.put("chunkX", context.center.x);
      event.put("chunkZ", context.center.z);
      event.put("scenario", SCENARIO);
      event.put("seed", SEED);
      event.put("targetChunkX", TARGET_CHUNK_X);
      event.put("targetChunkZ", TARGET_CHUNK_Z);
      event.put("targetRadius", TARGET_RADIUS);
      event.put("stepIndex", context.stepIndex);
      event.put("featureIndex", context.featureIndex);
      event.put("configuredFeature", context.configuredFeature);
      event.put("featureType", context.featureType);
      event.put("treeConfiguredFeature", treeConfiguredFeature);
      event.put("treeFeatureType", treeFeatureType);
      event.put("originX", origin.getX());
      event.put("originY", origin.getY());
      event.put("originZ", origin.getZ());
      event.put("targetLocalX", origin.getX() - (TARGET_CHUNK_X << 4));
      event.put("targetLocalZ", origin.getZ() - (TARGET_CHUNK_Z << 4));
      event.put("randomCountBefore", randomCount(random));
      event.put("thread", Thread.currentThread().getName());
      CURRENT_TREE_CANDIDATE.set(new TreeCandidateProbe(event));
      return true;
   }

   public static void finishTreeCandidate(boolean placed, Random random, Throwable error) {
      TreeCandidateProbe probe = CURRENT_TREE_CANDIDATE.get();
      if (probe == null) {
         return;
      }

      CURRENT_TREE_CANDIDATE.remove();
      probe.event.put("placed", placed);
      probe.event.put("randomCountAfter", randomCount(random));
      if (error != null) {
         probe.event.put("error", error.getClass().getName() + ": " + error.getMessage());
      }

      synchronized (McloneSchedulerTraceRecorder.class) {
         if (!written) {
            probe.event.put("sequence", ++sequence);
            EVENTS.add(probe.event);
         }
      }
   }

   public static boolean hasActiveTreeCandidate() {
      return CURRENT_TREE_CANDIDATE.get() != null;
   }

   public static void recordTreeCandidateParameters(int treeHeight, int foliageHeight, int trunkHeight, int foliageRadius) {
      TreeCandidateProbe probe = CURRENT_TREE_CANDIDATE.get();
      if (probe == null) {
         return;
      }

      probe.event.put("treeHeight", treeHeight);
      probe.event.put("foliageHeight", foliageHeight);
      probe.event.put("trunkHeight", trunkHeight);
      probe.event.put("foliageRadius", foliageRadius);
   }

   public static void recordTreeCandidateConfiguration(String trunkPlacer, String foliagePlacer, String minimumSize) {
      TreeCandidateProbe probe = CURRENT_TREE_CANDIDATE.get();
      if (probe == null) {
         return;
      }

      probe.event.put("trunkPlacer", trunkPlacer);
      probe.event.put("foliagePlacer", foliagePlacer);
      probe.event.put("minimumSize", minimumSize);
   }

   public static void recordTreeCandidateSaplingState(BlockState state, boolean canSurvive) {
      TreeCandidateProbe probe = CURRENT_TREE_CANDIDATE.get();
      if (probe == null) {
         return;
      }

      probe.event.put("saplingState", blockName(state));
      probe.event.put("saplingCanSurvive", canSurvive);
   }

   public static void recordTreeCandidateHeightCheck(int maxFreeTreeHeight, OptionalInt minClippedHeight) {
      TreeCandidateProbe probe = CURRENT_TREE_CANDIDATE.get();
      if (probe == null) {
         return;
      }

      probe.event.put("maxFreeTreeHeight", maxFreeTreeHeight);
      if (minClippedHeight.isPresent()) {
         probe.event.put("minClippedHeight", minClippedHeight.getAsInt());
      }
   }

   public static void recordTreeCandidateBlockedPosition(
      BlockPos pos,
      BlockState state,
      boolean vine,
      int heightOffset,
      int radius
   ) {
      TreeCandidateProbe probe = CURRENT_TREE_CANDIDATE.get();
      if (probe == null) {
         return;
      }

      probe.event.put("blockedX", pos.getX());
      probe.event.put("blockedY", pos.getY());
      probe.event.put("blockedZ", pos.getZ());
      probe.event.put("blockedBlock", blockName(state));
      probe.event.put("blockedByVine", vine);
      probe.event.put("blockedHeightOffset", heightOffset);
      probe.event.put("blockedRadius", radius);
   }

   public static void recordTreeCandidateAccepted(int maxFreeTreeHeight) {
      TreeCandidateProbe probe = CURRENT_TREE_CANDIDATE.get();
      if (probe == null) {
         return;
      }

      probe.event.put("acceptedMaxFreeTreeHeight", maxFreeTreeHeight);
   }

   public static void recordTreeCandidateRejected(String reason) {
      TreeCandidateProbe probe = CURRENT_TREE_CANDIDATE.get();
      if (probe == null) {
         return;
      }

      probe.event.put("rejectReason", reason);
   }

   public static void recordTreeCandidateBlockCounts(int trunkBlocks, int foliageBlocks, int decoratorBlocks, boolean finalResult) {
      TreeCandidateProbe probe = CURRENT_TREE_CANDIDATE.get();
      if (probe == null) {
         return;
      }

      probe.event.put("trunkBlocks", trunkBlocks);
      probe.event.put("foliageBlocks", foliageBlocks);
      probe.event.put("decoratorBlocks", decoratorBlocks);
      probe.event.put("finalTreeResult", finalResult);
   }

   static synchronized void recordFeatureProbe(
      WorldGenRegion region,
      int stepIndex,
      int featureIndex,
      String configuredFeature,
      String featureType,
      int randomCount
   ) {
      if (!ENABLED || written || !hasFeatureProbes()) {
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
      ChunkAccess target = region.getChunk(TARGET_CHUNK_X, TARGET_CHUNK_Z, ChunkStatus.EMPTY, false);
      appendProbeBlocks(event, target);
      appendTargetTreeBlocks(event, target);
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
      if (target == null || PROBE_BLOCKS.isEmpty()) {
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

   private static void appendTargetTreeBlocks(Map<String, Object> event, ChunkAccess target) {
      if (!PROBE_TARGET_TREE_BLOCKS || target == null) {
         return;
      }

      List<Map<String, Object>> treeBlocks = new ArrayList<>();
      for (int y = target.getMinBuildHeight(); y < target.getMaxBuildHeight(); y++) {
         for (int localZ = 0; localZ < 16; localZ++) {
            for (int localX = 0; localX < 16; localX++) {
               BlockPos pos = new BlockPos((TARGET_CHUNK_X << 4) + localX, y, (TARGET_CHUNK_Z << 4) + localZ);
               BlockState state = target.getBlockState(pos);
               if (state.getBlock() != Blocks.SPRUCE_LOG && state.getBlock() != Blocks.SPRUCE_LEAVES) {
                  continue;
               }

               Map<String, Object> row = new LinkedHashMap<>();
               row.put("localX", localX);
               row.put("y", y);
               row.put("localZ", localZ);
               row.put("block", Registry.BLOCK.getKey(state.getBlock()).toString());
               treeBlocks.add(row);
            }
         }
      }
      event.put("targetTreeBlocks", treeBlocks);
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
         "ChunkStatus.generate:task_complete",
         "ChunkStatus.applyBiomeDecorationWithFeatureProbes:feature_probe",
         "ConfiguredFeature.place:tree_candidate",
         "TreeFeature.doPlace:tree_candidate_details"
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

   private static boolean matchesTreeProbeFilter(FeatureProbeContext context) {
      if (TREE_PROBE_CENTER_X != Integer.MIN_VALUE && context.center.x != TREE_PROBE_CENTER_X) {
         return false;
      }
      if (TREE_PROBE_CENTER_Z != Integer.MIN_VALUE && context.center.z != TREE_PROBE_CENTER_Z) {
         return false;
      }
      if (TREE_PROBE_STEP_INDEX >= 0 && context.stepIndex != TREE_PROBE_STEP_INDEX) {
         return false;
      }
      if (TREE_PROBE_FEATURE_INDEX >= 0 && context.featureIndex != TREE_PROBE_FEATURE_INDEX) {
         return false;
      }
      return true;
   }

   private static Object randomCount(Random random) {
      if (random instanceof WorldgenRandom) {
         return ((WorldgenRandom)random).getCount();
      }
      return null;
   }

   private static String blockName(BlockState state) {
      if (state == null) {
         return "unknown";
      }
      return Registry.BLOCK.getKey(state.getBlock()).toString();
   }

   private static final class FeatureProbeContext {
      final ChunkPos center;
      final int stepIndex;
      final int featureIndex;
      final String configuredFeature;
      final String featureType;

      FeatureProbeContext(ChunkPos center, int stepIndex, int featureIndex, String configuredFeature, String featureType) {
         this.center = center;
         this.stepIndex = stepIndex;
         this.featureIndex = featureIndex;
         this.configuredFeature = configuredFeature;
         this.featureType = featureType;
      }
   }

   private static final class TreeCandidateProbe {
      final Map<String, Object> event;

      TreeCandidateProbe(Map<String, Object> event) {
         this.event = event;
      }
   }
}
