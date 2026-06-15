import com.google.gson.Gson;
import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.util.LinkedHashMap;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.TimeUnit;
import java.util.function.Function;
import java.util.function.Supplier;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.Registry;
import net.minecraft.core.SectionPos;
import net.minecraft.data.BuiltinRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.client.renderer.chunk.VisGraph;
import net.minecraft.client.renderer.chunk.VisibilitySet;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.LightLayer;
import net.minecraft.world.level.NoiseColumn;
import net.minecraft.world.level.TickList;
import net.minecraft.world.level.TickPriority;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeGenerationSettings;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.biome.BiomeSource;
import net.minecraft.world.level.biome.FuzzyOffsetConstantColumnBiomeZoomer;
import net.minecraft.world.level.biome.OverworldBiomeSource;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.ChunkStatus;
import net.minecraft.world.level.chunk.DataLayer;
import net.minecraft.world.level.chunk.LightChunkGetter;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.NoiseModifier;
import net.minecraft.world.level.levelgen.NoiseSampler;
import net.minecraft.world.level.levelgen.NoiseSettings;
import net.minecraft.world.level.levelgen.GenerationStep;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.SimpleRandomSource;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.feature.ConfiguredFeature;
import net.minecraft.world.level.levelgen.feature.StructureFeature;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.level.chunk.UpgradeData;
import net.minecraft.world.level.levelgen.synth.BlendedNoise;
import net.minecraft.world.level.levelgen.synth.ImprovedNoise;
import net.minecraft.world.level.levelgen.synth.NormalNoise;
import net.minecraft.world.level.levelgen.synth.PerlinNoise;
import net.minecraft.world.level.levelgen.synth.PerlinSimplexNoise;
import net.minecraft.world.level.levelgen.synth.SimplexNoise;
import net.minecraft.world.level.levelgen.synth.SurfaceNoise;
import net.minecraft.world.level.material.Fluid;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.lighting.LayerLightEventListener;
import net.minecraft.world.level.lighting.LevelLightEngine;

public final class OracleDumper {
   private static final String MINECRAFT_VERSION = "1.17.1";
   private static final String RANDOM_SOURCE_CLASS = "net.minecraft.world.level.levelgen.SimpleRandomSource";
   private static final String RANDOM_SOURCE_ALIAS = "LegacyRandomSource";
   private static final String BLENDED_NOISE_CLASS = BlendedNoise.class.getName();
   private static final String IMPROVED_NOISE_CLASS = ImprovedNoise.class.getName();
   private static final String NOISE_SAMPLER_CLASS = NoiseSampler.class.getName();
   private static final String NORMAL_NOISE_CLASS = NormalNoise.class.getName();
   private static final String OVERWORLD_BIOME_SOURCE_CLASS = OverworldBiomeSource.class.getName();
   private static final String PERLIN_NOISE_CLASS = PerlinNoise.class.getName();
   private static final String PERLIN_SIMPLEX_NOISE_CLASS = PerlinSimplexNoise.class.getName();
   private static final String SIMPLEX_NOISE_CLASS = SimplexNoise.class.getName();
   private static final String WORLDGEN_RANDOM_CLASS = WorldgenRandom.class.getName();
   private static final int[] NOISE_SAMPLER_PATTERN_AXIS = createIntAxis(-2, 1, 5);
   private static final Gson GSON = new Gson();
   private static final double[] NOISE_X_COORDS = createAxis(-2.5, 0.5, 11);
   private static final double[] NOISE_Y_COORDS = createAxis(-1.875, 0.375, 11);
   private static final double[] NOISE_Z_COORDS = createAxis(-3.125, 0.625, 11);
   private static final int[] SURFACE_NOISE_OCTAVES = createIntAxis(-3, 1, 4);
   private static final int[] BLENDED_LIMIT_OCTAVES = createIntAxis(-15, 1, 16);
   private static final int[] BLENDED_MAIN_OCTAVES = createIntAxis(-7, 1, 8);
   private static final int SYNTHETIC_LIGHT_MIN_Y = -16;
   private static final int SYNTHETIC_LIGHT_HEIGHT = 48;
   private static final int SYNTHETIC_LIGHT_MIN_SECTION_Y = -1;
   private static final int SYNTHETIC_LIGHT_MAX_SECTION_Y_EXCLUSIVE = 2;
   private static final String[] SURFACE_AND_CARVED_PALETTE = new String[]{
      "minecraft:air",
      "minecraft:stone",
      "minecraft:water",
      "minecraft:bedrock",
      "minecraft:grass_block",
      "minecraft:dirt",
      "minecraft:sand",
      "minecraft:gravel",
      "minecraft:snow",
      "minecraft:lava",
      "minecraft:granite",
      "minecraft:diorite",
      "minecraft:andesite",
      "minecraft:coarse_dirt",
      "minecraft:podzol",
      "minecraft:mycelium",
      "minecraft:terracotta",
      "minecraft:white_terracotta",
      "minecraft:orange_terracotta",
      "minecraft:magenta_terracotta",
      "minecraft:light_blue_terracotta",
      "minecraft:yellow_terracotta",
      "minecraft:lime_terracotta",
      "minecraft:pink_terracotta",
      "minecraft:gray_terracotta",
      "minecraft:light_gray_terracotta",
      "minecraft:cyan_terracotta",
      "minecraft:purple_terracotta",
      "minecraft:blue_terracotta",
      "minecraft:brown_terracotta",
      "minecraft:green_terracotta",
      "minecraft:red_terracotta",
      "minecraft:black_terracotta",
      "minecraft:sandstone",
      "minecraft:red_sandstone",
      "minecraft:packed_ice",
      "minecraft:obsidian",
      "minecraft:magma_block",
      "minecraft:red_sand",
      "minecraft:ice",
      "minecraft:snow_block"
   };
   private static final BlendedNoiseSampleParameters[] BLENDED_NOISE_SAMPLE_SETS = new BlendedNoiseSampleParameters[]{
      blendedNoiseSampleParameters("overworld", new String[]{"overworld", "amplified"}, 0.9999999814507745, 0.9999999814507745, 80.0, 160.0),
      blendedNoiseSampleParameters("nether", new String[]{"nether", "caves"}, 1.0, 3.0, 80.0, 60.0),
      blendedNoiseSampleParameters("end", new String[]{"end", "floating_islands"}, 2.0, 1.0, 80.0, 160.0)
   };
   private static final BiomePatternSpec[] OVERWORLD_NOISE_SAMPLER_PATTERNS = new BiomePatternSpec[]{
      new BiomePatternSpec(
         "constantPlains",
         new String[]{
            "plains", "plains", "plains", "plains", "plains",
            "plains", "plains", "plains", "plains", "plains",
            "plains", "plains", "plains", "plains", "plains",
            "plains", "plains", "plains", "plains", "plains",
            "plains", "plains", "plains", "plains", "plains"
         }
      ),
      new BiomePatternSpec(
         "mixedOverworld",
         new String[]{
            "ocean", "plains", "forest", "mountains", "badlands",
            "plains", "forest", "mountains", "badlands", "ocean",
            "forest", "mountains", "badlands", "ocean", "plains",
            "mountains", "badlands", "ocean", "plains", "forest",
            "badlands", "ocean", "plains", "forest", "mountains"
         }
      )
   };

   private OracleDumper() {
   }

   public static void main(String[] args) {
      try {
         run(args);
      } catch (IllegalArgumentException error) {
         System.err.println("error: " + error.getMessage());
         System.err.println();
         printUsage();
         System.exit(1);
      } catch (Exception error) {
         error.printStackTrace(System.err);
         System.exit(1);
      }
   }

   private static void run(String[] args) throws Exception {
      if (args.length == 0 || "--help".equals(args[0]) || "-h".equals(args[0])) {
         printUsage();
         return;
      }

      String module = args[0];
      Map<String, String> options = parseOptions(args, 1);
      long seed = parseSeed(requireOption(options, "seed"));

      String json;
      switch (module) {
         case "prng":
            json = dumpPrng(seed, parseCount(requireOption(options, "count")));
            break;
         case "biome":
            json = dumpBiome(seed, options);
            break;
         case "noise":
            json = dumpNoise(seed, options);
            break;
         case "terrain-chunk":
            json = dumpTerrainChunk(seed, parseInteger(requireOption(options, "chunk-x"), "chunk-x"), parseInteger(requireOption(options, "chunk-z"), "chunk-z"));
            break;
         case "surface-chunk":
            json = dumpSurfaceChunk(seed, parseInteger(requireOption(options, "chunk-x"), "chunk-x"), parseInteger(requireOption(options, "chunk-z"), "chunk-z"));
            break;
         case "carved-chunk":
            json = dumpCarvedChunk(seed, parseInteger(requireOption(options, "chunk-x"), "chunk-x"), parseInteger(requireOption(options, "chunk-z"), "chunk-z"));
            break;
         case "liquid-carved-chunk":
            json = dumpLiquidCarvedChunk(seed, parseInteger(requireOption(options, "chunk-x"), "chunk-x"), parseInteger(requireOption(options, "chunk-z"), "chunk-z"));
            break;
         case "feature-order-trace":
            json = dumpFeatureOrderTrace(
               seed,
               parseInteger(requireOption(options, "chunk-x"), "chunk-x"),
               parseInteger(requireOption(options, "chunk-z"), "chunk-z"),
               parseBooleanOption(options, "generate-structures", false)
            );
            break;
         case "scheduler-trace":
            json = dumpSchedulerTrace(
               seed,
               parseInteger(requireOption(options, "chunk-x"), "chunk-x"),
               parseInteger(requireOption(options, "chunk-z"), "chunk-z"),
               options
            );
            break;
         case "visgraph":
            json = dumpVisGraph();
            break;
         case "synthetic-light":
            json = dumpSyntheticLight();
            break;
         default:
            throw new IllegalArgumentException("unsupported module '" + module + "'");
      }

      System.out.print(json);
   }

   private static Map<String, String> parseOptions(String[] args, int startIndex) {
      Map<String, String> options = new LinkedHashMap<>();

      for (int index = startIndex; index < args.length; index++) {
         String argument = args[index];
         if (!argument.startsWith("--")) {
            throw new IllegalArgumentException("expected option starting with '--', got '" + argument + "'");
         }

         String name = argument.substring(2);
         if (name.isEmpty()) {
            throw new IllegalArgumentException("empty option name");
         }

         if (index + 1 >= args.length) {
            throw new IllegalArgumentException("missing value for '--" + name + "'");
         }

         String value = args[++index];
         if (options.put(name, value) != null) {
            throw new IllegalArgumentException("duplicate option '--" + name + "'");
         }
      }

      return options;
   }

   private static String requireOption(Map<String, String> options, String name) {
      String value = options.get(name);
      if (value == null) {
         throw new IllegalArgumentException("missing required option '--" + name + "'");
      }

      return value;
   }

   private static long parseSeed(String value) {
      try {
         return Long.decode(value);
      } catch (NumberFormatException error) {
         throw new IllegalArgumentException("invalid seed '" + value + "'");
      }
   }

   private static int parseCount(String value) {
      try {
         int count = Integer.parseInt(value);
         if (count <= 0) {
            throw new IllegalArgumentException("count must be positive");
         }

         return count;
      } catch (NumberFormatException error) {
         throw new IllegalArgumentException("invalid count '" + value + "'");
      }
   }

   private static int parseInteger(String value, String name) {
      try {
         return Integer.parseInt(value);
      } catch (NumberFormatException error) {
         throw new IllegalArgumentException("invalid " + name + " '" + value + "'");
      }
   }

   private static boolean parseBooleanOption(Map<String, String> options, String name, boolean defaultValue) {
      String value = options.get(name);
      if (value == null) {
         return defaultValue;
      }

      if ("true".equals(value)) {
         return true;
      }

      if ("false".equals(value)) {
         return false;
      }

      throw new IllegalArgumentException("invalid " + name + " '" + value + "'");
   }

   private static int parseIntegerOption(Map<String, String> options, String name, int defaultValue) {
      String value = options.get(name);
      return value == null ? defaultValue : parseInteger(value, name);
   }

   private static String dumpPrng(long seed, int count) {
      StringBuilder json = new StringBuilder(512 + count * 48);
      json.append("{\n");
      appendField(json, 1, "module", "prng", true);
      appendField(json, 1, "minecraftVersion", MINECRAFT_VERSION, true);
      appendField(json, 1, "randomSourceClass", RANDOM_SOURCE_CLASS, true);
      appendField(json, 1, "randomSourceAlias", RANDOM_SOURCE_ALIAS, true);
      appendField(json, 1, "seed", Long.toString(seed), true);
      appendNumberField(json, 1, "count", Integer.toString(count), true);
      appendField(json, 1, "sequenceMode", "fresh-instance-per-method", true);
      json.append("  \"wireFormat\": {\n");
      appendField(json, 2, "nextInt", "number", true);
      appendField(json, 2, "nextLong", "signed-decimal-string", true);
      appendField(json, 2, "nextDouble", "number", false);
      json.append("  },\n");
      json.append("  \"nextInt\": ");
      appendIntArray(json, seed, count);
      json.append(",\n");
      json.append("  \"nextLong\": ");
      appendLongArray(json, seed, count);
      json.append(",\n");
      json.append("  \"nextDouble\": ");
      appendDoubleArray(json, seed, count);
      json.append('\n');
      json.append("}\n");
      return json.toString();
   }

   private static String dumpNoise(long seed, Map<String, String> options) throws Exception {
      String className = options.get("class");
      if (className == null || className.isEmpty() || "ImprovedNoise".equals(className)) {
         return dumpImprovedNoise(seed);
      }

      switch (className) {
         case "NoiseSampler":
            return dumpNoiseSampler(seed, requireOption(options, "preset"), loadSampleGrid2D(requireOption(options, "samples2d")));
         case "NormalNoise":
            return dumpNormalNoise(
               seed,
               parseInteger(requireOption(options, "first-octave"), "first-octave"),
               parseAmplitudes(requireOption(options, "amplitudes")),
               loadSampleGrid(requireOption(options, "samples"))
            );
         case "PerlinNoise":
            return dumpPerlinNoise(seed, parseOctaves(requireOption(options, "octaves")), loadSampleGrid(requireOption(options, "samples")));
         case "PerlinSimplexNoise":
            return dumpPerlinSimplexNoise(seed, parseOctaves(requireOption(options, "octaves")), loadSampleGrid2D(requireOption(options, "samples2d")));
         case "SimplexNoise":
            return dumpSimplexNoise(seed, loadSampleGrid2D(requireOption(options, "samples2d")), loadSampleGrid(requireOption(options, "samples3d")));
         case "BlendedNoise":
            return dumpBlendedNoise(seed, loadIntegerSampleGrid(requireOption(options, "samples")));
         default:
            throw new IllegalArgumentException("unsupported noise class '" + className + "'");
      }
   }

   private static String dumpBiome(long seed, Map<String, String> options) throws Exception {
      String className = options.get("class");
      if (className == null || className.isEmpty() || "OverworldBiomeSource".equals(className)) {
         return dumpOverworldBiomeSource(
            seed,
            parseBooleanOption(options, "legacy-biome-init-layer", false),
            parseBooleanOption(options, "large-biomes", false),
            loadSampleGrid2D(requireOption(options, "samples2d"))
         );
      }

      throw new IllegalArgumentException("unsupported biome class '" + className + "'");
   }

   private static String dumpVisGraph() {
      Map<String, Object> root = new LinkedHashMap<>();
      root.put("module", "visgraph");
      root.put("minecraftVersion", MINECRAFT_VERSION);
      root.put("visGraphClass", VisGraph.class.getName());
      root.put("visibilitySetClass", VisibilitySet.class.getName());
      root.put("faceOrder", directionNames());

      List<Map<String, Object>> cases = new ArrayList<>();
      cases.add(dumpVisGraphCase("empty", new ArrayList<>()));
      cases.add(dumpVisGraphCase("solid", solidCells()));
      cases.add(dumpVisGraphCase("lightlyOpaque", lightlyOpaqueCells()));
      cases.add(dumpVisGraphCase("xWall", xWallCells()));
      cases.add(dumpVisGraphCase("westEastTunnel", westEastTunnelCells()));
      cases.add(dumpVisGraphCase("enclosedCavity", enclosedCavityCells()));
      root.put("cases", cases);
      return GSON.toJson(root) + "\n";
   }

   private static Map<String, Object> dumpVisGraphCase(String name, List<int[]> opaqueCells) {
      VisGraph graph = new VisGraph();
      for (int[] cell : opaqueCells) {
         graph.setOpaque(new BlockPos(cell[0], cell[1], cell[2]));
      }
      VisibilitySet visibility = graph.resolve();

      Map<String, Object> result = new LinkedHashMap<>();
      result.put("name", name);
      result.put("opaqueCount", opaqueCells.size());
      result.put("opaque", opaqueCells);
      result.put("visibility", visibilityRows(visibility));
      return result;
   }

   private static String dumpSyntheticLight() throws Exception {
      java.io.PrintStream originalOut = System.out;
      try {
         System.setOut(System.err);
         SharedConstants.tryDetectVersion();
         Bootstrap.bootStrap();
      } finally {
         System.setOut(originalOut);
      }

      Map<String, Object> root = new LinkedHashMap<>();
      root.put("module", "synthetic-light");
      root.put("minecraftVersion", MINECRAFT_VERSION);
      root.put("lightEngineClass", LevelLightEngine.class.getName());

      Map<String, Object> level = new LinkedHashMap<>();
      level.put("minY", SYNTHETIC_LIGHT_MIN_Y);
      level.put("height", SYNTHETIC_LIGHT_HEIGHT);
      level.put("minSectionY", SYNTHETIC_LIGHT_MIN_SECTION_Y);
      level.put("maxSectionYExclusive", SYNTHETIC_LIGHT_MAX_SECTION_Y_EXCLUSIVE);
      root.put("level", level);

      Map<String, Object> blockPalette = new LinkedHashMap<>();
      blockPalette.put("air", "minecraft:air");
      blockPalette.put("stone", "minecraft:stone");
      blockPalette.put("lava", "minecraft:lava");
      root.put("blockPalette", blockPalette);

      List<Map<String, Object>> cases = new ArrayList<>();
      cases.add(dumpSyntheticSkyCase(
         "openColumn",
         chunkList(new int[]{0, 0}),
         new ArrayList<>(),
         sampleList(new int[]{4, 15, 4}, new int[]{4, 0, 4}, new int[]{0, 0, 0}, new int[]{15, 0, 15})
      ));
      cases.add(dumpSyntheticSkyCase(
         "fullRoof",
         chunkList(new int[]{0, 0}),
         fullRoofCells(14),
         sampleList(new int[]{8, 15, 8}, new int[]{8, 14, 8}, new int[]{8, 13, 8}, new int[]{0, 13, 0})
      ));
      cases.add(dumpSyntheticSkyCase(
         "fullRoofEastOpenNeighbor",
         chunkList(new int[]{0, 0}, new int[]{1, 0}),
         fullRoofCells(14),
         sampleList(new int[]{16, 13, 8}, new int[]{15, 13, 8}, new int[]{14, 13, 8}, new int[]{8, 13, 8})
      ));
      cases.add(dumpSyntheticSkyCase(
         "singleOverhang",
         chunkList(new int[]{0, 0}),
         sampleList(new int[]{0, 14, 0}),
         sampleList(new int[]{0, 15, 0}, new int[]{0, 14, 0}, new int[]{0, 13, 0}, new int[]{1, 13, 0}, new int[]{1, 0, 0})
      ));
      cases.add(dumpSyntheticSkyCase(
         "stoneRoom",
         chunkList(new int[]{0, 0}),
         stoneRoomCells(3, 5, 0, 14, 3, 5),
         sampleList(new int[]{4, 15, 4}, new int[]{4, 14, 4}, new int[]{4, 13, 4}, new int[]{2, 13, 4}, new int[]{6, 13, 4})
      ));
      cases.add(dumpSyntheticSkyCase(
         "chunkBoundaryOverhang",
         chunkList(new int[]{0, 0}, new int[]{1, 0}),
         sampleList(new int[]{15, 14, 0}),
         sampleList(new int[]{15, 15, 0}, new int[]{15, 14, 0}, new int[]{15, 13, 0}, new int[]{16, 13, 0}, new int[]{16, 0, 0})
      ));
      root.put("cases", cases);

      List<Map<String, Object>> blockCases = new ArrayList<>();
      blockCases.add(dumpSyntheticBlockCase(
         "lavaOpen",
         chunkList(new int[]{0, 0}),
         new ArrayList<>(),
         sampleList(new int[]{1, 1, 1}),
         sampleList(new int[]{1, 1, 1}, new int[]{2, 1, 1}, new int[]{3, 1, 1}, new int[]{1, 2, 1}, new int[]{1, 0, 1})
      ));
      blockCases.add(dumpSyntheticBlockCase(
         "lavaBlockedByStone",
         chunkList(new int[]{0, 0}),
         sampleList(new int[]{2, 1, 1}),
         sampleList(new int[]{1, 1, 1}),
         sampleList(new int[]{1, 1, 1}, new int[]{2, 1, 1}, new int[]{3, 1, 1}, new int[]{1, 2, 1})
      ));
      blockCases.add(dumpSyntheticBlockCase(
         "lavaCrossChunk",
         chunkList(new int[]{0, 0}, new int[]{1, 0}),
         new ArrayList<>(),
         sampleList(new int[]{15, 1, 1}),
         sampleList(new int[]{15, 1, 1}, new int[]{16, 1, 1}, new int[]{17, 1, 1}, new int[]{14, 1, 1})
      ));
      root.put("blockCases", blockCases);

      return GSON.toJson(root) + "\n";
   }

   private static Map<String, Object> dumpSyntheticSkyCase(
      String name,
      List<int[]> chunks,
      List<int[]> opaqueCells,
      List<int[]> samplePositions
   ) {
      SyntheticLightLevel level = new SyntheticLightLevel(SYNTHETIC_LIGHT_MIN_Y, SYNTHETIC_LIGHT_HEIGHT);
      LevelLightEngine engine = new LevelLightEngine(level, false, true);

      for (int[] chunk : chunks) {
         level.addChunk(chunk[0], chunk[1]);
         for (int sectionY = SYNTHETIC_LIGHT_MIN_SECTION_Y; sectionY < SYNTHETIC_LIGHT_MAX_SECTION_Y_EXCLUSIVE; sectionY++) {
            engine.updateSectionStatus(SectionPos.of(chunk[0], sectionY, chunk[1]), false);
         }
      }

      for (int[] cell : opaqueCells) {
         level.setBlock(cell[0], cell[1], cell[2], Blocks.STONE.defaultBlockState());
      }

      for (int[] chunk : chunks) {
         engine.enableLightSources(new ChunkPos(chunk[0], chunk[1]), true);
      }
      runLightUntilIdle(engine);

      Map<String, Object> result = new LinkedHashMap<>();
      result.put("name", name);
      result.put("chunks", chunks);
      result.put("opaqueCount", opaqueCells.size());
      result.put("opaque", opaqueCells);

      List<Map<String, Object>> samples = new ArrayList<>();
      for (int[] position : samplePositions) {
         samples.add(lightSample(engine, position[0], position[1], position[2]));
      }
      result.put("samples", samples);

      List<Map<String, Object>> skySections = new ArrayList<>();
      for (int[] chunk : chunks) {
         for (int sectionY = SYNTHETIC_LIGHT_MIN_SECTION_Y; sectionY < SYNTHETIC_LIGHT_MAX_SECTION_Y_EXCLUSIVE; sectionY++) {
            skySections.add(lightSection(engine, LightLayer.SKY, chunk[0], sectionY, chunk[1]));
         }
      }
      result.put("skySections", skySections);
      return result;
   }

   private static Map<String, Object> dumpSyntheticBlockCase(
      String name,
      List<int[]> chunks,
      List<int[]> opaqueCells,
      List<int[]> lavaCells,
      List<int[]> samplePositions
   ) {
      SyntheticLightLevel level = new SyntheticLightLevel(SYNTHETIC_LIGHT_MIN_Y, SYNTHETIC_LIGHT_HEIGHT);
      LevelLightEngine engine = new LevelLightEngine(level, true, false);

      for (int[] chunk : chunks) {
         level.addChunk(chunk[0], chunk[1]);
         for (int sectionY = SYNTHETIC_LIGHT_MIN_SECTION_Y; sectionY < SYNTHETIC_LIGHT_MAX_SECTION_Y_EXCLUSIVE; sectionY++) {
            engine.updateSectionStatus(SectionPos.of(chunk[0], sectionY, chunk[1]), false);
         }
      }

      for (int[] cell : opaqueCells) {
         level.setBlock(cell[0], cell[1], cell[2], Blocks.STONE.defaultBlockState());
      }

      BlockState lava = Blocks.LAVA.defaultBlockState();
      for (int[] cell : lavaCells) {
         BlockPos pos = new BlockPos(cell[0], cell[1], cell[2]);
         level.setBlock(cell[0], cell[1], cell[2], lava);
         engine.onBlockEmissionIncrease(pos, lava.getLightEmission());
      }
      runLightUntilIdle(engine);

      Map<String, Object> result = new LinkedHashMap<>();
      result.put("name", name);
      result.put("chunks", chunks);
      result.put("opaqueCount", opaqueCells.size());
      result.put("opaque", opaqueCells);
      result.put("lava", lavaCells);

      List<Map<String, Object>> samples = new ArrayList<>();
      for (int[] position : samplePositions) {
         samples.add(lightSample(engine, position[0], position[1], position[2]));
      }
      result.put("samples", samples);

      List<Map<String, Object>> blockSections = new ArrayList<>();
      for (int[] chunk : chunks) {
         for (int sectionY = SYNTHETIC_LIGHT_MIN_SECTION_Y; sectionY < SYNTHETIC_LIGHT_MAX_SECTION_Y_EXCLUSIVE; sectionY++) {
            blockSections.add(lightSection(engine, LightLayer.BLOCK, chunk[0], sectionY, chunk[1]));
         }
      }
      result.put("blockSections", blockSections);
      return result;
   }

   private static void runLightUntilIdle(LevelLightEngine engine) {
      for (int pass = 0; pass < 100; pass++) {
         if (!engine.hasLightWork()) {
            return;
         }
         engine.runUpdates(100_000, true, false);
      }
      throw new IllegalStateException("LevelLightEngine did not become idle");
   }

   private static Map<String, Object> lightSample(LevelLightEngine engine, int x, int y, int z) {
      BlockPos pos = new BlockPos(x, y, z);
      Map<String, Object> sample = new LinkedHashMap<>();
      sample.put("x", x);
      sample.put("y", y);
      sample.put("z", z);
      sample.put("sky", engine.getLayerListener(LightLayer.SKY).getLightValue(pos));
      sample.put("block", engine.getLayerListener(LightLayer.BLOCK).getLightValue(pos));
      sample.put("raw0", engine.getRawBrightness(pos, 0));
      return sample;
   }

   private static Map<String, Object> lightSection(LevelLightEngine engine, LightLayer layer, int sectionX, int sectionY, int sectionZ) {
      LayerLightEventListener listener = engine.getLayerListener(layer);
      DataLayer data = listener.getDataLayerData(SectionPos.of(sectionX, sectionY, sectionZ));
      byte[] bytes = data == null ? new byte[2048] : data.getData();

      Map<String, Object> section = new LinkedHashMap<>();
      section.put("sectionX", sectionX);
      section.put("sectionY", sectionY);
      section.put("sectionZ", sectionZ);
      section.put("dataHex", bytesToHex(bytes));
      return section;
   }

   private static List<int[]> chunkList(int[]... chunks) {
      List<int[]> result = new ArrayList<>();
      for (int[] chunk : chunks) {
         result.add(chunk);
      }
      return result;
   }

   private static List<int[]> sampleList(int[]... samples) {
      List<int[]> result = new ArrayList<>();
      for (int[] sample : samples) {
         result.add(sample);
      }
      return result;
   }

   private static List<int[]> fullRoofCells(int y) {
      List<int[]> cells = new ArrayList<>();
      for (int z = 0; z < 16; z++) {
         for (int x = 0; x < 16; x++) {
            cells.add(new int[]{x, y, z});
         }
      }
      return cells;
   }

   private static List<int[]> stoneRoomCells(int minX, int maxX, int minY, int maxY, int minZ, int maxZ) {
      List<int[]> cells = new ArrayList<>();
      for (int y = minY; y <= maxY; y++) {
         for (int z = minZ; z <= maxZ; z++) {
            for (int x = minX; x <= maxX; x++) {
               boolean wall = x == minX || x == maxX || z == minZ || z == maxZ || y == maxY;
               if (wall) {
                  cells.add(new int[]{x, y, z});
               }
            }
         }
      }
      return cells;
   }

   private static String bytesToHex(byte[] bytes) {
      char[] result = new char[bytes.length * 2];
      char[] alphabet = "0123456789abcdef".toCharArray();
      for (int index = 0; index < bytes.length; index++) {
         int value = bytes[index] & 0xFF;
         result[index * 2] = alphabet[value >>> 4];
         result[index * 2 + 1] = alphabet[value & 15];
      }
      return new String(result);
   }

   private static final class SyntheticLightLevel implements BlockGetter, LightChunkGetter {
      private final int minY;
      private final int height;
      private final Set<Long> loadedChunks = new HashSet<>();
      private final Map<Long, BlockState> blocks = new HashMap<>();

      SyntheticLightLevel(int minY, int height) {
         this.minY = minY;
         this.height = height;
      }

      void addChunk(int chunkX, int chunkZ) {
         this.loadedChunks.add(ChunkPos.asLong(chunkX, chunkZ));
      }

      void setBlock(int x, int y, int z, BlockState state) {
         long key = BlockPos.asLong(x, y, z);
         if (state.isAir()) {
            this.blocks.remove(key);
         } else {
            this.blocks.put(key, state);
         }
      }

      @Override
      public BlockGetter getChunkForLighting(int chunkX, int chunkZ) {
         return this.loadedChunks.contains(ChunkPos.asLong(chunkX, chunkZ)) ? this : null;
      }

      @Override
      public BlockGetter getLevel() {
         return this;
      }

      @Override
      public BlockEntity getBlockEntity(BlockPos pos) {
         return null;
      }

      @Override
      public BlockState getBlockState(BlockPos pos) {
         if (this.isOutsideBuildHeight(pos)) {
            return Blocks.AIR.defaultBlockState();
         }
         return this.blocks.getOrDefault(pos.asLong(), Blocks.AIR.defaultBlockState());
      }

      @Override
      public FluidState getFluidState(BlockPos pos) {
         return this.getBlockState(pos).getFluidState();
      }

      @Override
      public int getHeight() {
         return this.height;
      }

      @Override
      public int getMinBuildHeight() {
         return this.minY;
      }
   }

   private static String[] directionNames() {
      Direction[] directions = Direction.values();
      String[] names = new String[directions.length];
      for (int index = 0; index < directions.length; index++) {
         names[index] = directions[index].getName();
      }
      return names;
   }

   private static boolean[][] visibilityRows(VisibilitySet visibility) {
      Direction[] directions = Direction.values();
      boolean[][] rows = new boolean[directions.length][directions.length];
      for (Direction row : directions) {
         for (Direction column : directions) {
            rows[row.ordinal()][column.ordinal()] = visibility.visibilityBetween(row, column);
         }
      }
      return rows;
   }

   private static List<int[]> lightlyOpaqueCells() {
      List<int[]> cells = new ArrayList<>();
      for (int x = 0; x < 15; x++) {
         for (int z = 0; z < 15; z++) {
            cells.add(new int[]{x, 0, z});
         }
      }
      return cells;
   }

   private static List<int[]> xWallCells() {
      List<int[]> cells = new ArrayList<>();
      for (int y = 0; y < 16; y++) {
         for (int z = 0; z < 16; z++) {
            cells.add(new int[]{8, y, z});
         }
      }
      return cells;
   }

   private static List<int[]> westEastTunnelCells() {
      List<int[]> cells = new ArrayList<>();
      for (int y = 0; y < 16; y++) {
         for (int z = 0; z < 16; z++) {
            for (int x = 0; x < 16; x++) {
               if (y != 8 || z != 8) {
                  cells.add(new int[]{x, y, z});
               }
            }
         }
      }
      return cells;
   }

   private static List<int[]> enclosedCavityCells() {
      List<int[]> cells = new ArrayList<>();
      for (int y = 0; y < 16; y++) {
         for (int z = 0; z < 16; z++) {
            for (int x = 0; x < 16; x++) {
               if (x != 8 || y != 8 || z != 8) {
                  cells.add(new int[]{x, y, z});
               }
            }
         }
      }
      return cells;
   }

   private static List<int[]> solidCells() {
      List<int[]> cells = new ArrayList<>();
      for (int y = 0; y < 16; y++) {
         for (int z = 0; z < 16; z++) {
            for (int x = 0; x < 16; x++) {
               cells.add(new int[]{x, y, z});
            }
         }
      }
      return cells;
   }

   private static String dumpImprovedNoise(long seed) {
      SimpleRandomSource random = new SimpleRandomSource(seed);
      ImprovedNoise noise = new ImprovedNoise(random);
      int sampleCount = NOISE_X_COORDS.length * NOISE_Y_COORDS.length * NOISE_Z_COORDS.length;
      StringBuilder json = new StringBuilder(2048 + sampleCount * 28);
      json.append("{\n");
      appendField(json, 1, "module", "noise", true);
      appendField(json, 1, "minecraftVersion", MINECRAFT_VERSION, true);
      appendField(json, 1, "noiseClass", IMPROVED_NOISE_CLASS, true);
      appendField(json, 1, "randomSourceClass", RANDOM_SOURCE_CLASS, true);
      appendField(json, 1, "randomSourceAlias", RANDOM_SOURCE_ALIAS, true);
      appendField(json, 1, "noiseMethod", "noise(x,y,z)", true);
      appendField(json, 1, "gridOrder", "x-major,y-major,z-minor", true);
      appendField(json, 1, "seed", Long.toString(seed), true);
      appendNumberField(json, 1, "sampleCount", Integer.toString(sampleCount), true);
      json.append("  \"wireFormat\": {\n");
      appendField(json, 2, "coordinates", "number", true);
      appendField(json, 2, "values", "number", false);
      json.append("  },\n");
      json.append("  \"offsets\": {\n");
      appendNumberField(json, 2, "xo", Double.toString(noise.xo), true);
      appendNumberField(json, 2, "yo", Double.toString(noise.yo), true);
      appendNumberField(json, 2, "zo", Double.toString(noise.zo), false);
      json.append("  },\n");
      json.append("  \"x\": ");
      appendDoubleArray(json, NOISE_X_COORDS);
      json.append(",\n");
      json.append("  \"y\": ");
      appendDoubleArray(json, NOISE_Y_COORDS);
      json.append(",\n");
      json.append("  \"z\": ");
      appendDoubleArray(json, NOISE_Z_COORDS);
      json.append(",\n");
      json.append("  \"values\": ");
      appendNoiseValueArray(json, noise);
      json.append('\n');
      json.append("}\n");
      return json.toString();
   }

   private static String dumpPerlinNoise(long seed, int[] octaves, SampleGrid sampleGrid) {
      PerlinNoise noise = new PerlinNoise(new SimpleRandomSource(seed), toIntegerList(octaves));
      int sampleCount = sampleGrid.x.length * sampleGrid.y.length * sampleGrid.z.length;
      StringBuilder json = new StringBuilder(2048 + sampleCount * 28);
      json.append("{\n");
      appendField(json, 1, "module", "noise", true);
      appendField(json, 1, "minecraftVersion", MINECRAFT_VERSION, true);
      appendField(json, 1, "noiseClass", PERLIN_NOISE_CLASS, true);
      appendField(json, 1, "randomSourceClass", RANDOM_SOURCE_CLASS, true);
      appendField(json, 1, "randomSourceAlias", RANDOM_SOURCE_ALIAS, true);
      appendField(json, 1, "noiseMethod", "getValue(x,y,z)", true);
      appendField(json, 1, "gridOrder", sampleGrid.gridOrder, true);
      appendField(json, 1, "seed", Long.toString(seed), true);
      appendNumberField(json, 1, "sampleCount", Integer.toString(sampleCount), true);
      json.append("  \"octaves\": ");
      appendIntArray(json, octaves);
      json.append(",\n");
      json.append("  \"wireFormat\": {\n");
      appendField(json, 2, "coordinates", "number", true);
      appendField(json, 2, "values", "number", false);
      json.append("  },\n");
      json.append("  \"x\": ");
      appendDoubleArray(json, sampleGrid.x);
      json.append(",\n");
      json.append("  \"y\": ");
      appendDoubleArray(json, sampleGrid.y);
      json.append(",\n");
      json.append("  \"z\": ");
      appendDoubleArray(json, sampleGrid.z);
      json.append(",\n");
      json.append("  \"values\": ");
      appendNoiseValueArray(json, sampleGrid, noise::getValue);
      json.append('\n');
      json.append("}\n");
      return json.toString();
   }

   private static String dumpNormalNoise(long seed, int firstOctave, double[] amplitudes, SampleGrid sampleGrid) {
      NormalNoise noise = NormalNoise.create(new SimpleRandomSource(seed), firstOctave, amplitudes);
      int sampleCount = sampleGrid.x.length * sampleGrid.y.length * sampleGrid.z.length;
      StringBuilder json = new StringBuilder(2048 + sampleCount * 28);
      json.append("{\n");
      appendField(json, 1, "module", "noise", true);
      appendField(json, 1, "minecraftVersion", MINECRAFT_VERSION, true);
      appendField(json, 1, "noiseClass", NORMAL_NOISE_CLASS, true);
      appendField(json, 1, "randomSourceClass", RANDOM_SOURCE_CLASS, true);
      appendField(json, 1, "randomSourceAlias", RANDOM_SOURCE_ALIAS, true);
      appendField(json, 1, "noiseMethod", "getValue(x,y,z)", true);
      appendField(json, 1, "gridOrder", sampleGrid.gridOrder, true);
      appendField(json, 1, "seed", Long.toString(seed), true);
      appendNumberField(json, 1, "sampleCount", Integer.toString(sampleCount), true);
      appendNumberField(json, 1, "firstOctave", Integer.toString(firstOctave), true);
      indent(json, 1);
      json.append("\"amplitudes\": ");
      appendDoubleArray(json, amplitudes);
      json.append(",\n");
      json.append("  \"wireFormat\": {\n");
      appendField(json, 2, "coordinates", "number", true);
      appendField(json, 2, "values", "number", false);
      json.append("  },\n");
      json.append("  \"x\": ");
      appendDoubleArray(json, sampleGrid.x);
      json.append(",\n");
      json.append("  \"y\": ");
      appendDoubleArray(json, sampleGrid.y);
      json.append(",\n");
      json.append("  \"z\": ");
      appendDoubleArray(json, sampleGrid.z);
      json.append(",\n");
      json.append("  \"values\": ");
      appendNoiseValueArray(json, sampleGrid, noise::getValue);
      json.append('\n');
      json.append("}\n");
      return json.toString();
   }

   private static String dumpNoiseSampler(long seed, String preset, SampleGrid2D sampleGrid2D) {
      if (!"overworld".equals(preset)) {
         throw new IllegalArgumentException("unsupported NoiseSampler preset '" + preset + "'");
      }

      SharedConstants.tryDetectVersion();
      java.io.PrintStream originalOut = Bootstrap.STDOUT;
      java.io.PrintStream originalErr = System.err;
      Bootstrap.bootStrap();
      System.setOut(originalOut);
      System.setErr(originalErr);
      NoiseGeneratorSettings generatorSettings = BuiltinRegistries.NOISE_GENERATOR_SETTINGS.getOrThrow(NoiseGeneratorSettings.OVERWORLD);
      NoiseSettings noiseSettings = generatorSettings.noiseSettings();
      int cellWidth = noiseSettings.noiseSizeHorizontal() * 4;
      int cellHeight = noiseSettings.noiseSizeVertical() * 4;
      int cellCountY = noiseSettings.height() / cellHeight;
      int biomeY = generatorSettings.seaLevel();
      int minCellY = Math.floorDiv(noiseSettings.minY(), cellHeight);
      int columnValueCount = cellCountY + 1;
      int[] sampleX = requireIntegerAxis(sampleGrid2D.x, "x");
      int[] sampleZ = requireIntegerAxis(sampleGrid2D.z, "z");
      int sampleCount = sampleX.length * sampleZ.length * columnValueCount;
      StringBuilder json = new StringBuilder(4096 + OVERWORLD_NOISE_SAMPLER_PATTERNS.length * sampleCount * 28);
      json.append("{\n");
      appendField(json, 1, "module", "noise", true);
      appendField(json, 1, "minecraftVersion", MINECRAFT_VERSION, true);
      appendField(json, 1, "noiseClass", NOISE_SAMPLER_CLASS, true);
      appendField(json, 1, "randomSourceClass", WORLDGEN_RANDOM_CLASS, true);
      appendField(json, 1, "settingsPreset", preset, true);
      appendField(json, 1, "noiseModifier", "PASSTHROUGH", true);
      appendField(json, 1, "seed", Long.toString(seed), true);
      appendNumberField(json, 1, "cellWidth", Integer.toString(cellWidth), true);
      appendNumberField(json, 1, "cellHeight", Integer.toString(cellHeight), true);
      appendNumberField(json, 1, "cellCountY", Integer.toString(cellCountY), true);
      appendNumberField(json, 1, "biomeY", Integer.toString(biomeY), true);
      appendNumberField(json, 1, "minCellY", Integer.toString(minCellY), true);
      appendNumberField(json, 1, "columnValueCount", Integer.toString(columnValueCount), true);
      json.append("  \"wireFormat\": {\n");
      appendField(json, 2, "coordinates", "integer", true);
      appendField(json, 2, "biomeKeys", "string", true);
      appendField(json, 2, "biomeFactors", "number", true);
      appendField(json, 2, "values", "number", false);
      json.append("  },\n");
      appendNoiseSettings(json, noiseSettings);
      json.append(",\n");
      json.append("  \"sampleSets\": {\n");

      for (int index = 0; index < OVERWORLD_NOISE_SAMPLER_PATTERNS.length; index++) {
         appendNoiseSamplerSampleSet(
            json,
            2,
            OVERWORLD_NOISE_SAMPLER_PATTERNS[index],
            seed,
            noiseSettings,
            cellWidth,
            cellHeight,
            cellCountY,
            biomeY,
            minCellY,
            columnValueCount,
            sampleX,
            sampleZ,
            sampleCount,
            index < OVERWORLD_NOISE_SAMPLER_PATTERNS.length - 1
         );
      }

      json.append("  }\n");
      json.append("}\n");
      return json.toString();
   }

   private static String dumpTerrainChunk(long seed, int chunkX, int chunkZ) {
      SharedConstants.tryDetectVersion();
      java.io.PrintStream originalOut = Bootstrap.STDOUT;
      java.io.PrintStream originalErr = System.err;
      Bootstrap.bootStrap();
      System.setOut(originalOut);
      System.setErr(originalErr);

      OverworldBiomeSource biomeSource = new OverworldBiomeSource(seed, false, false, BuiltinRegistries.BIOME);
      NoiseBasedChunkGenerator generator = new NoiseBasedChunkGenerator(
         biomeSource,
         seed,
         () -> BuiltinRegistries.NOISE_GENERATOR_SETTINGS.getOrThrow(NoiseGeneratorSettings.OVERWORLD)
      );
      int minY = generator.getMinY();
      int height = generator.getGenDepth();
      int[] blocks = new int[height * 16 * 16];
      int minBlockX = chunkX * 16;
      int minBlockZ = chunkZ * 16;
      BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
      LevelHeightAccessor level = new LevelHeightAccessor() {
         @Override
         public int getHeight() {
            return height;
         }

         @Override
         public int getMinBuildHeight() {
            return minY;
         }
      };

      for (int localZ = 0; localZ < 16; localZ++) {
         for (int localX = 0; localX < 16; localX++) {
            int x = minBlockX + localX;
            int z = minBlockZ + localZ;
            NoiseColumn column = generator.getBaseColumn(x, z, level);
            for (int y = minY; y < minY + height; y++) {
               BlockState state = column.getBlockState(pos.set(x, y, z));
               blocks[((y - minY) << 8) | (localZ << 4) | localX] = terrainBlockId(state);
            }
         }
      }

      applyBottomBedrock(blocks, minY, chunkX, chunkZ);

      Map<String, Object> json = new LinkedHashMap<>();
      json.put("module", "terrain-chunk");
      json.put("minecraftVersion", MINECRAFT_VERSION);
      json.put("generatorClass", NoiseBasedChunkGenerator.class.getName());
      json.put("seed", Long.toString(seed));
      json.put("chunkX", chunkX);
      json.put("chunkZ", chunkZ);
      json.put("minY", minY);
      json.put("height", height);
      json.put("blockOrder", "y-major,z-major,x-minor");
      json.put("palette", new String[]{"minecraft:air", "minecraft:stone", "minecraft:water", "minecraft:bedrock"});
      json.put("blocks", blocks);
      return GSON.toJson(json);
   }

   private static String dumpSurfaceChunk(long seed, int chunkX, int chunkZ) {
      SharedConstants.tryDetectVersion();
      java.io.PrintStream originalOut = Bootstrap.STDOUT;
      java.io.PrintStream originalErr = System.err;
      Bootstrap.bootStrap();
      System.setOut(originalOut);
      System.setErr(originalErr);

      OverworldBiomeSource biomeSource = new OverworldBiomeSource(seed, false, false, BuiltinRegistries.BIOME);
      NoiseGeneratorSettings generatorSettings = BuiltinRegistries.NOISE_GENERATOR_SETTINGS.getOrThrow(NoiseGeneratorSettings.OVERWORLD);
      NoiseBasedChunkGenerator generator = new NoiseBasedChunkGenerator(biomeSource, seed, () -> generatorSettings);
      int minY = generator.getMinY();
      int height = generator.getGenDepth();
      ProtoChunk chunk = createProtoChunk(chunkX, chunkZ, minY, height);

      fillChunkFromBaseColumns(generator, chunk, minY, height);
      chunk.setStatus(ChunkStatus.NOISE);
      buildSurfaceAndBedrock(seed, biomeSource, generatorSettings, chunk);
      chunk.setStatus(ChunkStatus.SURFACE);

      Map<String, Object> json = new LinkedHashMap<>();
      json.put("module", "surface-chunk");
      json.put("minecraftVersion", MINECRAFT_VERSION);
      json.put("generatorClass", NoiseBasedChunkGenerator.class.getName());
      json.put("seed", Long.toString(seed));
      json.put("chunkX", chunkX);
      json.put("chunkZ", chunkZ);
      json.put("minY", minY);
      json.put("height", height);
      json.put("blockOrder", "y-major,z-major,x-minor");
      json.put("palette", SURFACE_AND_CARVED_PALETTE);
      json.put("blocks", collectChunkBlocks(chunk, minY, height));
      return GSON.toJson(json);
   }

   private static String dumpCarvedChunk(long seed, int chunkX, int chunkZ) {
      return dumpCarvedChunk(seed, chunkX, chunkZ, false, "carved-chunk");
   }

   private static String dumpLiquidCarvedChunk(long seed, int chunkX, int chunkZ) {
      return dumpCarvedChunk(seed, chunkX, chunkZ, true, "liquid-carved-chunk");
   }

   private static String dumpCarvedChunk(long seed, int chunkX, int chunkZ, boolean includeLiquidStep, String moduleName) {
      SharedConstants.tryDetectVersion();
      java.io.PrintStream originalOut = Bootstrap.STDOUT;
      java.io.PrintStream originalErr = System.err;
      Bootstrap.bootStrap();
      System.setOut(originalOut);
      System.setErr(originalErr);

      OverworldBiomeSource biomeSource = new OverworldBiomeSource(seed, false, false, BuiltinRegistries.BIOME);
      NoiseGeneratorSettings generatorSettings = BuiltinRegistries.NOISE_GENERATOR_SETTINGS.getOrThrow(NoiseGeneratorSettings.OVERWORLD);
      NoiseBasedChunkGenerator generator = new NoiseBasedChunkGenerator(biomeSource, seed, () -> generatorSettings);
      int minY = generator.getMinY();
      int height = generator.getGenDepth();
      ProtoChunk chunk = createProtoChunk(chunkX, chunkZ, minY, height);
      BiomeManager biomeManager = new BiomeManager(biomeSource, BiomeManager.obfuscateSeed(seed), FuzzyOffsetConstantColumnBiomeZoomer.INSTANCE);

      fillChunkFromBaseColumns(generator, chunk, minY, height);
      chunk.setStatus(ChunkStatus.NOISE);
      buildSurfaceAndBedrock(seed, biomeSource, generatorSettings, chunk);
      chunk.setStatus(ChunkStatus.SURFACE);
      generator.applyCarvers(seed, biomeManager, chunk, GenerationStep.Carving.AIR);
      if (includeLiquidStep) {
         generator.applyCarvers(seed, biomeManager, chunk, GenerationStep.Carving.LIQUID);
      }
      chunk.setStatus(ChunkStatus.CARVERS);

      Map<String, Object> json = new LinkedHashMap<>();
      json.put("module", moduleName);
      json.put("minecraftVersion", MINECRAFT_VERSION);
      json.put("generatorClass", NoiseBasedChunkGenerator.class.getName());
      json.put("seed", Long.toString(seed));
      json.put("chunkX", chunkX);
      json.put("chunkZ", chunkZ);
      json.put("minY", minY);
      json.put("height", height);
      json.put("blockOrder", "y-major,z-major,x-minor");
      json.put("palette", SURFACE_AND_CARVED_PALETTE);
      json.put("blocks", collectChunkBlocks(chunk, minY, height));
      json.put("blockTicks", collectScheduledBlockTicks(chunk));
      json.put("liquidTicks", collectScheduledLiquidTicks(chunk));
      return GSON.toJson(json);
   }

   private static String dumpFeatureOrderTrace(long seed, int chunkX, int chunkZ, boolean generateStructures) throws Exception {
      SharedConstants.tryDetectVersion();
      java.io.PrintStream originalOut = Bootstrap.STDOUT;
      java.io.PrintStream originalErr = System.err;
      Bootstrap.bootStrap();
      System.setOut(originalOut);
      System.setErr(originalErr);

      OverworldBiomeSource biomeSource = new OverworldBiomeSource(seed, false, false, BuiltinRegistries.BIOME);
      NoiseBasedChunkGenerator generator = new NoiseBasedChunkGenerator(
         biomeSource,
         seed,
         () -> BuiltinRegistries.NOISE_GENERATOR_SETTINGS.getOrThrow(NoiseGeneratorSettings.OVERWORLD)
      );
      ChunkPos chunkPos = new ChunkPos(chunkX, chunkZ);
      int minBlockX = chunkPos.getMinBlockX();
      int minBlockZ = chunkPos.getMinBlockZ();
      Biome biome = biomeSource.getPrimaryBiome(chunkPos);
      WorldgenRandom random = new WorldgenRandom();
      long decorationSeed = random.setDecorationSeed(seed, minBlockX, minBlockZ);
      BiomeGenerationSettings generationSettings = biome.getGenerationSettings();
      List<List<Supplier<ConfiguredFeature<?, ?>>>> features = generationSettings.features();
      Map<Integer, List<StructureFeature<?>>> structuresByStep = generateStructures ? getStructuresByDecorationStep(biome) : Collections.emptyMap();
      List<Map<String, Object>> entries = new ArrayList<>();

      for (int stepIndex = 0; stepIndex < GenerationStep.Decoration.values().length; stepIndex++) {
         int featureIndex = 0;
         for (StructureFeature<?> structure : structuresByStep.getOrDefault(stepIndex, Collections.emptyList())) {
            long featureSeed = random.setFeatureSeed(decorationSeed, featureIndex, stepIndex);
            Map<String, Object> entry = new LinkedHashMap<>();
            entry.put("kind", "structure");
            entry.put("step", GenerationStep.Decoration.values()[stepIndex].name());
            entry.put("stepIndex", stepIndex);
            entry.put("featureIndex", featureIndex);
            entry.put("featureSeed", Long.toString(featureSeed));
            entry.put("structure", Registry.STRUCTURE_FEATURE.getKey(structure).toString());
            entries.add(entry);
            featureIndex++;
         }

         if (features.size() > stepIndex) {
            for (Supplier<ConfiguredFeature<?, ?>> featureSupplier : features.get(stepIndex)) {
               ConfiguredFeature<?, ?> feature = featureSupplier.get();
               long featureSeed = random.setFeatureSeed(decorationSeed, featureIndex, stepIndex);
               Map<String, Object> entry = new LinkedHashMap<>();
               entry.put("kind", "configured_feature");
               entry.put("step", GenerationStep.Decoration.values()[stepIndex].name());
               entry.put("stepIndex", stepIndex);
               entry.put("featureIndex", featureIndex);
               entry.put("featureSeed", Long.toString(featureSeed));
               entry.put("configuredFeature", configuredFeatureName(feature));
               entry.put("featureType", Registry.FEATURE.getKey(feature.feature()).toString());
               entries.add(entry);
               featureIndex++;
            }
         }
      }

      Map<String, Object> json = new LinkedHashMap<>();
      json.put("module", "feature-order-trace");
      json.put("minecraftVersion", MINECRAFT_VERSION);
      json.put("generatorClass", generator.getClass().getName());
      json.put("seed", Long.toString(seed));
      json.put("chunkX", chunkX);
      json.put("chunkZ", chunkZ);
      json.put("minBlockX", minBlockX);
      json.put("minBlockZ", minBlockZ);
      json.put("generateStructures", generateStructures);
      json.put("primaryBiome", BuiltinRegistries.BIOME.getKey(biome).toString());
      json.put("decorationSeed", Long.toString(decorationSeed));
      json.put("entries", entries);
      return GSON.toJson(json);
   }

   private static String dumpSchedulerTrace(long seed, int chunkX, int chunkZ, Map<String, String> options) throws Exception {
      int targetRadius = parseIntegerOption(options, "target-radius", 1);
      int recordRadius = parseIntegerOption(options, "record-radius", targetRadius);
      int timeoutSeconds = parseIntegerOption(options, "timeout-seconds", 180);
      if (targetRadius < 0) {
         throw new IllegalArgumentException("target-radius must be non-negative");
      }
      if (recordRadius < targetRadius) {
         throw new IllegalArgumentException("record-radius must be >= target-radius");
      }
      if (timeoutSeconds <= 0) {
         throw new IllegalArgumentException("timeout-seconds must be positive");
      }

      String scenario = options.getOrDefault("scenario", "spawn_bootstrap");
      String stopStatus = options.getOrDefault("stop-status", "features");
      String probeBlocks = options.getOrDefault("probe-blocks", "");
      boolean probeTargetTreeBlocks = parseBooleanOption(options, "probe-target-tree-blocks", false);
      boolean probeTreeCandidates = parseBooleanOption(options, "probe-tree-candidates", false);
      boolean probeOrePlacements = parseBooleanOption(options, "probe-ore-placements", false);
      boolean dumpChunks = parseBooleanOption(options, "dump-chunks", false);
      boolean dumpOnlyTargetChunk = parseBooleanOption(options, "dump-only-target-chunk", false);
      boolean generateStructures = parseBooleanOption(options, "generate-structures", true);
      Path tempDir = Files.createTempDirectory("mclone-scheduler-trace-");
      Path output = tempDir.resolve("scheduler-trace.json");
      Files.writeString(tempDir.resolve("eula.txt"), "eula=true\n", StandardCharsets.UTF_8);
      Files.writeString(tempDir.resolve("server.properties"), schedulerTraceServerProperties(seed, generateStructures), StandardCharsets.UTF_8);

      List<String> command = new ArrayList<>();
      command.add(Path.of(System.getProperty("java.home"), "bin", "java").toString());
      command.add("-XX:+UnlockExperimentalVMOptions");
      command.add("-XX:hashCode=3");
      command.add("-XX:ActiveProcessorCount=2");
      command.add("-Xms256m");
      command.add("-Xmx2g");
      command.add("-Djava.awt.headless=true");
      command.add("-Dmclone.schedulerTrace.enabled=true");
      command.add("-Dmclone.schedulerTrace.haltOnComplete=true");
      command.add("-Dmclone.schedulerTrace.seed=" + seed);
      command.add("-Dmclone.schedulerTrace.chunkX=" + chunkX);
      command.add("-Dmclone.schedulerTrace.chunkZ=" + chunkZ);
      command.add("-Dmclone.schedulerTrace.radius=" + targetRadius);
      command.add("-Dmclone.schedulerTrace.recordRadius=" + recordRadius);
      command.add("-Dmclone.schedulerTrace.scenario=" + scenario);
      command.add("-Dmclone.schedulerTrace.stopStatus=" + stopStatus);
      command.add("-Dmclone.schedulerTrace.probeBlocks=" + probeBlocks);
      command.add("-Dmclone.schedulerTrace.probeTargetTreeBlocks=" + probeTargetTreeBlocks);
      command.add("-Dmclone.schedulerTrace.probeTreeCandidates=" + probeTreeCandidates);
      command.add("-Dmclone.schedulerTrace.probeOrePlacements=" + probeOrePlacements);
      command.add("-Dmclone.schedulerTrace.dumpChunks=" + dumpChunks);
      command.add("-Dmclone.schedulerTrace.dumpOnlyTargetChunk=" + dumpOnlyTargetChunk);
      addOptionalSystemProperty(command, options, "tree-probe-center-x", "mclone.schedulerTrace.treeProbeCenterX");
      addOptionalSystemProperty(command, options, "tree-probe-center-z", "mclone.schedulerTrace.treeProbeCenterZ");
      addOptionalSystemProperty(command, options, "tree-probe-step-index", "mclone.schedulerTrace.treeProbeStepIndex");
      addOptionalSystemProperty(command, options, "tree-probe-feature-index", "mclone.schedulerTrace.treeProbeFeatureIndex");
      addOptionalSystemProperty(command, options, "ore-probe-center-x", "mclone.schedulerTrace.oreProbeCenterX");
      addOptionalSystemProperty(command, options, "ore-probe-center-z", "mclone.schedulerTrace.oreProbeCenterZ");
      addOptionalSystemProperty(command, options, "ore-probe-step-index", "mclone.schedulerTrace.oreProbeStepIndex");
      addOptionalSystemProperty(command, options, "ore-probe-feature-index", "mclone.schedulerTrace.oreProbeFeatureIndex");
      command.add("-Dmclone.schedulerTrace.identityHashCodeMode=3");
      command.add("-Dmclone.schedulerTrace.output=" + output);
      command.add("-cp");
      command.add(System.getProperty("java.class.path"));
      command.add("net.minecraft.server.Main");
      command.add("--nogui");
      command.add("--universe");
      command.add(tempDir.toString());
      command.add("--world");
      command.add("world");
      command.add("--port");
      command.add("0");

      Process process = new ProcessBuilder(command).directory(tempDir.toFile()).redirectErrorStream(true).start();
      StringBuilder log = new StringBuilder();
      Thread logThread = new Thread(() -> {
         try (BufferedReader reader = new BufferedReader(new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8))) {
            String line;
            while ((line = reader.readLine()) != null) {
               appendBoundedLog(log, line);
            }
         } catch (IOException ignored) {
         }
      }, "scheduler-trace-server-log");
      logThread.setDaemon(true);
      logThread.start();

      boolean finished = process.waitFor(timeoutSeconds, TimeUnit.SECONDS);
      if (!finished) {
         process.destroyForcibly();
         throw new IllegalStateException("scheduler trace server timed out after " + timeoutSeconds + "s\n" + log);
      }

      logThread.join(TimeUnit.SECONDS.toMillis(5));
      int exitCode = process.exitValue();
      if (!Files.exists(output)) {
         throw new IllegalStateException("scheduler trace server exited " + exitCode + " before writing " + output + "\n" + log);
      }
      if (exitCode != 0) {
         throw new IllegalStateException("scheduler trace server exited " + exitCode + "\n" + log);
      }

      return Files.readString(output, StandardCharsets.UTF_8);
   }

   private static String schedulerTraceServerProperties(long seed, boolean generateStructures) {
      return "allow-flight=true\n"
         + "difficulty=peaceful\n"
         + "enable-command-block=false\n"
         + "enable-query=false\n"
         + "enable-rcon=false\n"
         + "force-gamemode=false\n"
         + "gamemode=survival\n"
         + "generate-structures=" + Boolean.toString(generateStructures) + "\n"
         + "level-name=world\n"
         + "level-seed=" + seed + "\n"
         + "max-players=1\n"
         + "max-tick-time=-1\n"
         + "motd=mclone scheduler trace oracle\n"
         + "online-mode=false\n"
         + "pvp=false\n"
         + "server-ip=127.0.0.1\n"
         + "server-port=0\n"
         + "spawn-protection=0\n"
         + "sync-chunk-writes=false\n"
         + "view-distance=3\n"
         + "white-list=false\n";
   }

   private static void addOptionalSystemProperty(List<String> command, Map<String, String> options, String optionName, String propertyName) {
      String value = options.get(optionName);
      if (value != null) {
         command.add("-D" + propertyName + "=" + value);
      }
   }

   private static void appendBoundedLog(StringBuilder log, String line) {
      synchronized (log) {
         log.append(line).append('\n');
         int maxLength = 24000;
         if (log.length() > maxLength) {
            log.delete(0, log.length() - maxLength);
         }
      }
   }

   private static String configuredFeatureName(ConfiguredFeature<?, ?> feature) {
      return BuiltinRegistries.CONFIGURED_FEATURE.getResourceKey(feature).map(Object::toString).orElseGet(feature::toString);
   }

   @SuppressWarnings("unchecked")
   private static Map<Integer, List<StructureFeature<?>>> getStructuresByDecorationStep(Biome biome) throws Exception {
      Field field = Biome.class.getDeclaredField("structuresByStep");
      field.setAccessible(true);
      return new HashMap<>((Map<Integer, List<StructureFeature<?>>>)field.get(biome));
   }

   private static int terrainBlockId(BlockState state) {
      String key = Registry.BLOCK.getKey(state.getBlock()).toString();
      switch (key) {
         case "minecraft:air":
            return 0;
         case "minecraft:stone":
            return 1;
         case "minecraft:water":
            return 2;
         case "minecraft:bedrock":
            return 3;
         default:
            throw new IllegalStateException("unexpected terrain block from Java oracle: " + key);
      }
   }

   private static int surfaceBlockId(BlockState state) {
      String key = Registry.BLOCK.getKey(state.getBlock()).toString();
      switch (key) {
         case "minecraft:air":
            return 0;
         case "minecraft:stone":
            return 1;
         case "minecraft:water":
            return 2;
         case "minecraft:bedrock":
            return 3;
         case "minecraft:grass_block":
            return 4;
         case "minecraft:dirt":
            return 5;
         case "minecraft:sand":
            return 6;
         case "minecraft:gravel":
            return 7;
         case "minecraft:snow":
            return 8;
         case "minecraft:lava":
            return 9;
         case "minecraft:granite":
            return 10;
         case "minecraft:diorite":
            return 11;
         case "minecraft:andesite":
            return 12;
         case "minecraft:coarse_dirt":
            return 13;
         case "minecraft:podzol":
            return 14;
         case "minecraft:mycelium":
            return 15;
         case "minecraft:terracotta":
            return 16;
         case "minecraft:white_terracotta":
            return 17;
         case "minecraft:orange_terracotta":
            return 18;
         case "minecraft:magenta_terracotta":
            return 19;
         case "minecraft:light_blue_terracotta":
            return 20;
         case "minecraft:yellow_terracotta":
            return 21;
         case "minecraft:lime_terracotta":
            return 22;
         case "minecraft:pink_terracotta":
            return 23;
         case "minecraft:gray_terracotta":
            return 24;
         case "minecraft:light_gray_terracotta":
            return 25;
         case "minecraft:cyan_terracotta":
            return 26;
         case "minecraft:purple_terracotta":
            return 27;
         case "minecraft:blue_terracotta":
            return 28;
         case "minecraft:brown_terracotta":
            return 29;
         case "minecraft:green_terracotta":
            return 30;
         case "minecraft:red_terracotta":
            return 31;
         case "minecraft:black_terracotta":
            return 32;
         case "minecraft:sandstone":
            return 33;
         case "minecraft:red_sandstone":
            return 34;
         case "minecraft:packed_ice":
            return 35;
         case "minecraft:obsidian":
            return 36;
         case "minecraft:magma_block":
            return 37;
         case "minecraft:red_sand":
            return 38;
         case "minecraft:ice":
            return 39;
         case "minecraft:snow_block":
            return 40;
         default:
            throw new IllegalStateException("unexpected surface-stage block from Java oracle: " + key);
      }
   }

   private static void applyBottomBedrock(int[] blocks, int minY, int chunkX, int chunkZ) {
      WorldgenRandom random = new WorldgenRandom();
      random.setBaseChunkSeed(chunkX, chunkZ);

      for (int localZ = 0; localZ < 16; localZ++) {
         for (int localX = 0; localX < 16; localX++) {
            for (int offset = 4; offset >= 0; offset--) {
               if (offset <= random.nextInt(5)) {
                  int y = minY + offset;
                  blocks[((y - minY) << 8) | (localZ << 4) | localX] = 3;
               }
            }
         }
      }
   }

   private static ProtoChunk createProtoChunk(int chunkX, int chunkZ, int minY, int height) {
      return new ProtoChunk(new ChunkPos(chunkX, chunkZ), UpgradeData.EMPTY, createLevelHeightAccessor(minY, height));
   }

   private static LevelHeightAccessor createLevelHeightAccessor(int minY, int height) {
      return new LevelHeightAccessor() {
         @Override
         public int getHeight() {
            return height;
         }

         @Override
         public int getMinBuildHeight() {
            return minY;
         }
      };
   }

   private static void fillChunkFromBaseColumns(NoiseBasedChunkGenerator generator, ProtoChunk chunk, int minY, int height) {
      ChunkPos chunkPos = chunk.getPos();
      int minBlockX = chunkPos.getMinBlockX();
      int minBlockZ = chunkPos.getMinBlockZ();
      BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
      LevelHeightAccessor level = createLevelHeightAccessor(minY, height);

      for (int localZ = 0; localZ < 16; localZ++) {
         for (int localX = 0; localX < 16; localX++) {
            int x = minBlockX + localX;
            int z = minBlockZ + localZ;
            NoiseColumn column = generator.getBaseColumn(x, z, level);
            for (int y = minY; y < minY + height; y++) {
               BlockState state = column.getBlockState(pos.set(x, y, z));
               if (!state.isAir()) {
                  chunk.setBlockState(pos, state, false);
               }
            }
         }
      }
   }

   private static void buildSurfaceAndBedrock(long seed, OverworldBiomeSource biomeSource, NoiseGeneratorSettings generatorSettings, ProtoChunk chunk) {
      WorldgenRandom random = new WorldgenRandom();
      ChunkPos chunkPos = chunk.getPos();
      random.setBaseChunkSeed(chunkPos.x, chunkPos.z);

      SurfaceNoise surfaceNoise = createSurfaceNoise(seed, generatorSettings.noiseSettings());
      BiomeManager biomeManager = new BiomeManager(biomeSource, BiomeManager.obfuscateSeed(seed), FuzzyOffsetConstantColumnBiomeZoomer.INSTANCE);
      int minBlockX = chunkPos.getMinBlockX();
      int minBlockZ = chunkPos.getMinBlockZ();
      int seaLevel = generatorSettings.seaLevel();
      int minSurfaceLevel = generatorSettings.getMinSurfaceLevel();
      BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();

      for (int localX = 0; localX < 16; localX++) {
         for (int localZ = 0; localZ < 16; localZ++) {
            int x = minBlockX + localX;
            int z = minBlockZ + localZ;
            int heightY = chunk.getHeight(Heightmap.Types.WORLD_SURFACE_WG, localX, localZ) + 1;
            double surfaceValue = surfaceNoise.getSurfaceNoiseValue(x * 0.0625, z * 0.0625, 0.0625, localX * 0.0625) * 15.0;
            Biome biome = biomeManager.getBiome(pos.set(x, heightY, z));
            biome.buildSurfaceAt(
               random,
               chunk,
               x,
               z,
               heightY,
               surfaceValue,
               generatorSettings.getDefaultBlock(),
               generatorSettings.getDefaultFluid(),
               seaLevel,
               minSurfaceLevel,
               seed
            );
         }
      }

      applyBedrock(chunk, random, generatorSettings);
   }

   private static SurfaceNoise createSurfaceNoise(long seed, NoiseSettings noiseSettings) {
      WorldgenRandom random = new WorldgenRandom(seed);
      new BlendedNoise(random);
      return noiseSettings.useSimplexSurfaceNoise()
         ? new PerlinSimplexNoise(random, toIntegerList(SURFACE_NOISE_OCTAVES))
         : new PerlinNoise(random, toIntegerList(SURFACE_NOISE_OCTAVES));
   }

   private static void applyBedrock(ChunkAccess chunk, WorldgenRandom random, NoiseGeneratorSettings generatorSettings) {
      BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
      ChunkPos chunkPos = chunk.getPos();
      int minBlockX = chunkPos.getMinBlockX();
      int minBlockZ = chunkPos.getMinBlockZ();
      int minY = generatorSettings.noiseSettings().minY();
      int floorY = minY + generatorSettings.getBedrockFloorPosition();
      int roofY = chunk.getHeight() - 1 + minY - generatorSettings.getBedrockRoofPosition();
      int minBuildHeight = chunk.getMinBuildHeight();
      int maxBuildHeight = chunk.getMaxBuildHeight();
      boolean hasRoof = roofY + 5 - 1 >= minBuildHeight && roofY < maxBuildHeight;
      boolean hasFloor = floorY + 5 - 1 >= minBuildHeight && floorY < maxBuildHeight;

      if (!hasRoof && !hasFloor) {
         return;
      }

      for (int localZ = 0; localZ < 16; localZ++) {
         for (int localX = 0; localX < 16; localX++) {
            int x = minBlockX + localX;
            int z = minBlockZ + localZ;

            if (hasRoof) {
               for (int offset = 0; offset < 5; offset++) {
                  if (offset <= random.nextInt(5)) {
                     chunk.setBlockState(pos.set(x, roofY - offset, z), Blocks.BEDROCK.defaultBlockState(), false);
                  }
               }
            }

            if (hasFloor) {
               for (int offset = 4; offset >= 0; offset--) {
                  if (offset <= random.nextInt(5)) {
                     chunk.setBlockState(pos.set(x, floorY + offset, z), Blocks.BEDROCK.defaultBlockState(), false);
                  }
               }
            }
         }
      }
   }

   private static int[] collectChunkBlocks(ChunkAccess chunk, int minY, int height) {
      int[] blocks = new int[height * 16 * 16];
      ChunkPos chunkPos = chunk.getPos();
      int minBlockX = chunkPos.getMinBlockX();
      int minBlockZ = chunkPos.getMinBlockZ();
      BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();

      for (int localZ = 0; localZ < 16; localZ++) {
         for (int localX = 0; localX < 16; localX++) {
            int x = minBlockX + localX;
            int z = minBlockZ + localZ;
            for (int y = minY; y < minY + height; y++) {
               blocks[((y - minY) << 8) | (localZ << 4) | localX] = surfaceBlockId(chunk.getBlockState(pos.set(x, y, z)));
            }
         }
      }

      return blocks;
   }

   private static List<Map<String, Object>> collectScheduledBlockTicks(ProtoChunk chunk) {
      ScheduledTickCollector<net.minecraft.world.level.block.Block> collector = new ScheduledTickCollector<>(Registry.BLOCK::getKey);
      chunk.getBlockTicks().copyOut(collector, pos -> chunk.getBlockState(pos).getBlock());
      return collector.toJson();
   }

   private static List<Map<String, Object>> collectScheduledLiquidTicks(ProtoChunk chunk) {
      ScheduledTickCollector<Fluid> collector = new ScheduledTickCollector<>(Registry.FLUID::getKey);
      chunk.getLiquidTicks().copyOut(collector, pos -> chunk.getFluidState(pos).getType());
      return collector.toJson();
   }

   private static final class ScheduledTickCollector<T> implements TickList<T> {
      private final Function<T, ResourceLocation> toId;
      private final List<Map<String, Object>> ticks = new ArrayList<>();

      private ScheduledTickCollector(Function<T, ResourceLocation> toId) {
         this.toId = toId;
      }

      @Override
      public boolean hasScheduledTick(BlockPos pos, T target) {
         return false;
      }

      @Override
      public void scheduleTick(BlockPos pos, T target, int delay, TickPriority priority) {
         Map<String, Object> tick = new LinkedHashMap<>();
         tick.put("x", pos.getX());
         tick.put("y", pos.getY());
         tick.put("z", pos.getZ());
         tick.put("target", this.toId.apply(target).toString());
         tick.put("delay", delay);
         this.ticks.add(tick);
      }

      @Override
      public boolean willTickThisTick(BlockPos pos, T target) {
         return false;
      }

      @Override
      public int size() {
         return this.ticks.size();
      }

      private List<Map<String, Object>> toJson() {
         return this.ticks;
      }
   }

   private static String dumpOverworldBiomeSource(long seed, boolean legacyBiomeInitLayer, boolean largeBiomes, SampleGrid2D sampleGrid2D) {
      SharedConstants.tryDetectVersion();
      java.io.PrintStream originalOut = Bootstrap.STDOUT;
      java.io.PrintStream originalErr = System.err;
      Bootstrap.bootStrap();
      System.setOut(originalOut);
      System.setErr(originalErr);

      int[] sampleX = requireIntegerAxis(sampleGrid2D.x, "x");
      int[] sampleZ = requireIntegerAxis(sampleGrid2D.z, "z");
      int sampleCount = sampleX.length * sampleZ.length;
      OverworldBiomeSource biomeSource = new OverworldBiomeSource(seed, legacyBiomeInitLayer, largeBiomes, BuiltinRegistries.BIOME);
      List<Biome> possibleBiomes = biomeSource.possibleBiomes();
      StringBuilder json = new StringBuilder(4096 + sampleCount * 96 + possibleBiomes.size() * 80);
      json.append("{\n");
      appendField(json, 1, "module", "biome", true);
      appendField(json, 1, "minecraftVersion", MINECRAFT_VERSION, true);
      appendField(json, 1, "biomeSourceClass", OVERWORLD_BIOME_SOURCE_CLASS, true);
      appendField(json, 1, "seed", Long.toString(seed), true);
      appendNumberField(json, 1, "legacyBiomeInitLayer", Boolean.toString(legacyBiomeInitLayer), true);
      appendNumberField(json, 1, "largeBiomes", Boolean.toString(largeBiomes), true);
      json.append("  \"wireFormat\": {\n");
      appendField(json, 2, "coordinates", "integer", true);
      appendField(json, 2, "biomeIds", "integer", true);
      appendField(json, 2, "biomeKeys", "string", true);
      appendField(json, 2, "biomeFactors", "number", false);
      json.append("  },\n");
      appendPossibleBiomes(json, possibleBiomes);
      json.append(",\n");
      json.append("  \"samples\": {\n");
      appendField(json, 2, "biomeMethod", "getNoiseBiome(x,0,z)", true);
      appendField(json, 2, "gridOrder", sampleGrid2D.gridOrder, true);
      appendNumberField(json, 2, "sampleY", "0", true);
      appendNumberField(json, 2, "sampleCount", Integer.toString(sampleCount), true);
      indent(json, 2);
      json.append("\"x\": ");
      appendIntArray(json, sampleX);
      json.append(",\n");
      indent(json, 2);
      json.append("\"z\": ");
      appendIntArray(json, sampleZ);
      json.append(",\n");
      indent(json, 2);
      json.append("\"ids\": ");
      appendBiomeSourceIdArray(json, biomeSource, sampleX, sampleZ);
      json.append(",\n");
      indent(json, 2);
      json.append("\"keys\": ");
      appendBiomeSourceKeyArray(json, biomeSource, sampleX, sampleZ);
      json.append(",\n");
      indent(json, 2);
      json.append("\"depths\": ");
      appendBiomeSourceFactorArray(json, biomeSource, sampleX, sampleZ, true);
      json.append(",\n");
      indent(json, 2);
      json.append("\"scales\": ");
      appendBiomeSourceFactorArray(json, biomeSource, sampleX, sampleZ, false);
      json.append('\n');
      json.append("  }\n");
      json.append("}\n");
      return json.toString();
   }

   private static String dumpPerlinSimplexNoise(long seed, int[] octaves, SampleGrid2D sampleGrid2D) {
      PerlinSimplexNoise noise = new PerlinSimplexNoise(new SimpleRandomSource(seed), toIntegerList(octaves));
      int sampleCount = sampleGrid2D.x.length * sampleGrid2D.z.length;
      StringBuilder json = new StringBuilder(3072 + sampleCount * 56);
      json.append("{\n");
      appendField(json, 1, "module", "noise", true);
      appendField(json, 1, "minecraftVersion", MINECRAFT_VERSION, true);
      appendField(json, 1, "noiseClass", PERLIN_SIMPLEX_NOISE_CLASS, true);
      appendField(json, 1, "randomSourceClass", RANDOM_SOURCE_CLASS, true);
      appendField(json, 1, "randomSourceAlias", RANDOM_SOURCE_ALIAS, true);
      appendField(json, 1, "seed", Long.toString(seed), true);
      json.append("  \"octaves\": ");
      appendIntArray(json, octaves);
      json.append(",\n");
      json.append("  \"wireFormat\": {\n");
      appendField(json, 2, "coordinates", "number", true);
      appendField(json, 2, "values", "number", false);
      json.append("  },\n");
      appendSampleSet2D(json, "samplesWithoutOffsets", "getValue(x,y,false)", sampleGrid2D, sampleCount, (x, y) -> noise.getValue(x, y, false), true);
      appendSampleSet2D(json, "samplesWithOffsets", "getValue(x,y,true)", sampleGrid2D, sampleCount, (x, y) -> noise.getValue(x, y, true), false);
      json.append("}\n");
      return json.toString();
   }

   private static String dumpSimplexNoise(long seed, SampleGrid2D sampleGrid2D, SampleGrid sampleGrid3D) {
      SimplexNoise noise = new SimplexNoise(new SimpleRandomSource(seed));
      int sampleCount2D = sampleGrid2D.x.length * sampleGrid2D.z.length;
      int sampleCount3D = sampleGrid3D.x.length * sampleGrid3D.y.length * sampleGrid3D.z.length;
      StringBuilder json = new StringBuilder(3072 + sampleCount2D * 28 + sampleCount3D * 28);
      json.append("{\n");
      appendField(json, 1, "module", "noise", true);
      appendField(json, 1, "minecraftVersion", MINECRAFT_VERSION, true);
      appendField(json, 1, "noiseClass", SIMPLEX_NOISE_CLASS, true);
      appendField(json, 1, "randomSourceClass", RANDOM_SOURCE_CLASS, true);
      appendField(json, 1, "randomSourceAlias", RANDOM_SOURCE_ALIAS, true);
      appendField(json, 1, "seed", Long.toString(seed), true);
      json.append("  \"wireFormat\": {\n");
      appendField(json, 2, "coordinates", "number", true);
      appendField(json, 2, "values", "number", false);
      json.append("  },\n");
      json.append("  \"offsets\": {\n");
      appendNumberField(json, 2, "xo", Double.toString(noise.xo), true);
      appendNumberField(json, 2, "yo", Double.toString(noise.yo), true);
      appendNumberField(json, 2, "zo", Double.toString(noise.zo), false);
      json.append("  },\n");
      appendSampleSet2D(json, "samples2d", "getValue(x,z)", sampleGrid2D, sampleCount2D, noise::getValue, true);
      appendSampleSet3D(json, "samples3d", "getValue(x,y,z)", sampleGrid3D, sampleCount3D, noise::getValue, false);
      json.append("}\n");
      return json.toString();
   }

   private static String dumpBlendedNoise(long seed, IntegerSampleGrid sampleGrid) {
      SimpleRandomSource random = new SimpleRandomSource(seed);
      PerlinNoise minLimitNoise = new PerlinNoise(random, toIntegerList(BLENDED_LIMIT_OCTAVES));
      PerlinNoise maxLimitNoise = new PerlinNoise(random, toIntegerList(BLENDED_LIMIT_OCTAVES));
      PerlinNoise mainNoise = new PerlinNoise(random, toIntegerList(BLENDED_MAIN_OCTAVES));
      BlendedNoise noise = new BlendedNoise(minLimitNoise, maxLimitNoise, mainNoise);
      int sampleCount = sampleGrid.x.length * sampleGrid.y.length * sampleGrid.z.length;
      StringBuilder json = new StringBuilder(4096 + BLENDED_NOISE_SAMPLE_SETS.length * sampleCount * 28);
      json.append("{\n");
      appendField(json, 1, "module", "noise", true);
      appendField(json, 1, "minecraftVersion", MINECRAFT_VERSION, true);
      appendField(json, 1, "noiseClass", BLENDED_NOISE_CLASS, true);
      appendField(json, 1, "randomSourceClass", RANDOM_SOURCE_CLASS, true);
      appendField(json, 1, "randomSourceAlias", RANDOM_SOURCE_ALIAS, true);
      appendField(json, 1, "noiseMethod", "sampleAndClampNoise(x,y,z,limitHorizontalScale,limitVerticalScale,mainHorizontalScale,mainVerticalScale)", true);
      appendField(json, 1, "seed", Long.toString(seed), true);
      json.append("  \"octaves\": {\n");
      indent(json, 2);
      json.append("\"limit\": ");
      appendIntArray(json, BLENDED_LIMIT_OCTAVES);
      json.append(",\n");
      indent(json, 2);
      json.append("\"main\": ");
      appendIntArray(json, BLENDED_MAIN_OCTAVES);
      json.append('\n');
      json.append("  },\n");
      json.append("  \"wireFormat\": {\n");
      appendField(json, 2, "coordinates", "integer", true);
      appendField(json, 2, "parameters", "number", true);
      appendField(json, 2, "values", "number", false);
      json.append("  },\n");
      json.append("  \"sampleSets\": {\n");

      for (int index = 0; index < BLENDED_NOISE_SAMPLE_SETS.length; index++) {
         appendBlendedSampleSet(
            json,
            2,
            BLENDED_NOISE_SAMPLE_SETS[index],
            sampleGrid,
            sampleCount,
            noise,
            mainNoise,
            index < BLENDED_NOISE_SAMPLE_SETS.length - 1
         );
      }

      json.append("  }\n");
      json.append("}\n");
      return json.toString();
   }

   private static void appendIntArray(StringBuilder json, long seed, int count) {
      SimpleRandomSource random = new SimpleRandomSource(seed);
      json.append('[');

      for (int index = 0; index < count; index++) {
         if (index > 0) {
            json.append(',');
         }

         json.append(random.nextInt());
      }

      json.append(']');
   }

   private static void appendLongArray(StringBuilder json, long seed, int count) {
      SimpleRandomSource random = new SimpleRandomSource(seed);
      json.append('[');

      for (int index = 0; index < count; index++) {
         if (index > 0) {
            json.append(',');
         }

         json.append('"');
         json.append(random.nextLong());
         json.append('"');
      }

      json.append(']');
   }

   private static void appendDoubleArray(StringBuilder json, long seed, int count) {
      SimpleRandomSource random = new SimpleRandomSource(seed);
      json.append('[');

      for (int index = 0; index < count; index++) {
         if (index > 0) {
            json.append(',');
         }

         json.append(Double.toString(random.nextDouble()));
      }

      json.append(']');
   }

   private static void appendIntArray(StringBuilder json, int[] values) {
      json.append('[');

      for (int index = 0; index < values.length; index++) {
         if (index > 0) {
            json.append(',');
         }

         json.append(values[index]);
      }

      json.append(']');
   }

   private static void appendDoubleArray(StringBuilder json, double[] values) {
      json.append('[');

      for (int index = 0; index < values.length; index++) {
         if (index > 0) {
            json.append(',');
         }

         json.append(Double.toString(values[index]));
      }

      json.append(']');
   }

   private static void appendPossibleBiomes(StringBuilder json, List<Biome> possibleBiomes) {
      indent(json, 1);
      json.append("\"possibleBiomes\": [\n");

      for (int index = 0; index < possibleBiomes.size(); index++) {
         Biome biome = possibleBiomes.get(index);
         indent(json, 2);
         json.append("{\n");
         appendNumberField(json, 3, "id", Integer.toString(BuiltinRegistries.BIOME.getId(biome)), true);
         appendField(json, 3, "key", BuiltinRegistries.BIOME.getKey(biome).toString(), true);
         appendNumberField(json, 3, "depth", Double.toString(biome.getDepth()), true);
         appendNumberField(json, 3, "scale", Double.toString(biome.getScale()), false);
         indent(json, 2);
         json.append('}');
         if (index < possibleBiomes.size() - 1) {
            json.append(',');
         }

         json.append('\n');
      }

      indent(json, 1);
      json.append(']');
   }

   private static void appendBiomeSourceIdArray(StringBuilder json, OverworldBiomeSource biomeSource, int[] sampleX, int[] sampleZ) {
      json.append('[');
      boolean first = true;

      for (int x : sampleX) {
         for (int z : sampleZ) {
            if (!first) {
               json.append(',');
            }

            first = false;
            json.append(BuiltinRegistries.BIOME.getId(biomeSource.getNoiseBiome(x, 0, z)));
         }
      }

      json.append(']');
   }

   private static void appendBiomeSourceKeyArray(StringBuilder json, OverworldBiomeSource biomeSource, int[] sampleX, int[] sampleZ) {
      json.append('[');
      boolean first = true;

      for (int x : sampleX) {
         for (int z : sampleZ) {
            if (!first) {
               json.append(',');
            }

            first = false;
            json.append('"');
            json.append(escapeJson(BuiltinRegistries.BIOME.getKey(biomeSource.getNoiseBiome(x, 0, z)).toString()));
            json.append('"');
         }
      }

      json.append(']');
   }

   private static void appendBiomeSourceFactorArray(StringBuilder json, OverworldBiomeSource biomeSource, int[] sampleX, int[] sampleZ, boolean useDepth) {
      json.append('[');
      boolean first = true;

      for (int x : sampleX) {
         for (int z : sampleZ) {
            if (!first) {
               json.append(',');
            }

            first = false;
            Biome biome = biomeSource.getNoiseBiome(x, 0, z);
            json.append(Double.toString(useDepth ? biome.getDepth() : biome.getScale()));
         }
      }

      json.append(']');
   }

   private static void appendStringArray(StringBuilder json, String[] values) {
      json.append('[');

      for (int index = 0; index < values.length; index++) {
         if (index > 0) {
            json.append(',');
         }

         json.append('"');
         json.append(escapeJson(values[index]));
         json.append('"');
      }

      json.append(']');
   }

   private static void appendNoiseValueArray(StringBuilder json, ImprovedNoise noise) {
      appendNoiseValueArray(json, defaultNoiseSampleGrid(), noise::noise);
   }

   private static void appendNoiseValueArray(StringBuilder json, SampleGrid sampleGrid, NoiseValueSampler3D noise) {
      json.append('[');
      boolean first = true;

      for (double x : sampleGrid.x) {
         for (double y : sampleGrid.y) {
            for (double z : sampleGrid.z) {
               if (!first) {
                  json.append(',');
               }

               first = false;
               json.append(Double.toString(noise.sample(x, y, z)));
            }
         }
      }

      json.append(']');
   }

   private static void appendNoiseValueArray(StringBuilder json, SampleGrid2D sampleGrid, NoiseValueSampler2D noise) {
      json.append('[');
      boolean first = true;

      for (double x : sampleGrid.x) {
         for (double z : sampleGrid.z) {
            if (!first) {
               json.append(',');
            }

            first = false;
            json.append(Double.toString(noise.sample(x, z)));
         }
      }

      json.append(']');
   }

   private static BlendFactorSummary appendBlendedNoiseValueArray(
      StringBuilder json,
      IntegerSampleGrid sampleGrid,
      BlendedNoise noise,
      PerlinNoise mainNoise,
      BlendedNoiseSampleParameters params
   ) {
      BlendFactorSummary summary = new BlendFactorSummary();
      json.append('[');
      boolean first = true;

      for (int x : sampleGrid.x) {
         for (int y : sampleGrid.y) {
            for (int z : sampleGrid.z) {
               if (!first) {
                  json.append(',');
               }

               first = false;
               double blendFactor = computeBlendedNoiseFactor(x, y, z, params, mainNoise);
               summary.record(blendFactor);
               json.append(
                  Double.toString(
                     noise.sampleAndClampNoise(
                        x,
                        y,
                        z,
                        params.limitHorizontalScale,
                        params.limitVerticalScale,
                        params.mainHorizontalScale,
                        params.mainVerticalScale
                     )
                  )
               );
            }
         }
      }

      json.append(']');
      return summary;
   }

   private static void appendSampleSet2D(StringBuilder json, String name, String noiseMethod, SampleGrid2D sampleGrid, int sampleCount, NoiseValueSampler2D noise, boolean trailingComma) {
      indent(json, 1);
      json.append('"');
      json.append(escapeJson(name));
      json.append("\": {\n");
      appendField(json, 2, "noiseMethod", noiseMethod, true);
      appendField(json, 2, "gridOrder", sampleGrid.gridOrder, true);
      appendNumberField(json, 2, "sampleCount", Integer.toString(sampleCount), true);
      indent(json, 2);
      json.append("\"x\": ");
      appendDoubleArray(json, sampleGrid.x);
      json.append(",\n");
      indent(json, 2);
      json.append("\"z\": ");
      appendDoubleArray(json, sampleGrid.z);
      json.append(",\n");
      indent(json, 2);
      json.append("\"values\": ");
      appendNoiseValueArray(json, sampleGrid, noise);
      json.append('\n');
      indent(json, 1);
      json.append('}');
      if (trailingComma) {
         json.append(',');
      }

      json.append('\n');
   }

   private static void appendSampleSet3D(StringBuilder json, String name, String noiseMethod, SampleGrid sampleGrid, int sampleCount, NoiseValueSampler3D noise, boolean trailingComma) {
      indent(json, 1);
      json.append('"');
      json.append(escapeJson(name));
      json.append("\": {\n");
      appendField(json, 2, "noiseMethod", noiseMethod, true);
      appendField(json, 2, "gridOrder", sampleGrid.gridOrder, true);
      appendNumberField(json, 2, "sampleCount", Integer.toString(sampleCount), true);
      indent(json, 2);
      json.append("\"x\": ");
      appendDoubleArray(json, sampleGrid.x);
      json.append(",\n");
      indent(json, 2);
      json.append("\"y\": ");
      appendDoubleArray(json, sampleGrid.y);
      json.append(",\n");
      indent(json, 2);
      json.append("\"z\": ");
      appendDoubleArray(json, sampleGrid.z);
      json.append(",\n");
      indent(json, 2);
      json.append("\"values\": ");
      appendNoiseValueArray(json, sampleGrid, noise);
      json.append('\n');
      indent(json, 1);
      json.append('}');
      if (trailingComma) {
         json.append(',');
      }

      json.append('\n');
   }

   private static void appendNoiseSettings(StringBuilder json, NoiseSettings noiseSettings) {
      indent(json, 1);
      json.append("\"noiseSettings\": {\n");
      appendNumberField(json, 2, "minY", Integer.toString(noiseSettings.minY()), true);
      appendNumberField(json, 2, "height", Integer.toString(noiseSettings.height()), true);
      indent(json, 2);
      json.append("\"sampling\": {\n");
      appendNumberField(json, 3, "xzScale", Double.toString(noiseSettings.noiseSamplingSettings().xzScale()), true);
      appendNumberField(json, 3, "yScale", Double.toString(noiseSettings.noiseSamplingSettings().yScale()), true);
      appendNumberField(json, 3, "xzFactor", Double.toString(noiseSettings.noiseSamplingSettings().xzFactor()), true);
      appendNumberField(json, 3, "yFactor", Double.toString(noiseSettings.noiseSamplingSettings().yFactor()), false);
      indent(json, 2);
      json.append("},\n");
      indent(json, 2);
      json.append("\"topSlide\": {\n");
      appendNumberField(json, 3, "target", Integer.toString(noiseSettings.topSlideSettings().target()), true);
      appendNumberField(json, 3, "size", Integer.toString(noiseSettings.topSlideSettings().size()), true);
      appendNumberField(json, 3, "offset", Integer.toString(noiseSettings.topSlideSettings().offset()), false);
      indent(json, 2);
      json.append("},\n");
      indent(json, 2);
      json.append("\"bottomSlide\": {\n");
      appendNumberField(json, 3, "target", Integer.toString(noiseSettings.bottomSlideSettings().target()), true);
      appendNumberField(json, 3, "size", Integer.toString(noiseSettings.bottomSlideSettings().size()), true);
      appendNumberField(json, 3, "offset", Integer.toString(noiseSettings.bottomSlideSettings().offset()), false);
      indent(json, 2);
      json.append("},\n");
      appendNumberField(json, 2, "noiseSizeHorizontal", Integer.toString(noiseSettings.noiseSizeHorizontal()), true);
      appendNumberField(json, 2, "noiseSizeVertical", Integer.toString(noiseSettings.noiseSizeVertical()), true);
      appendNumberField(json, 2, "densityFactor", Double.toString(noiseSettings.densityFactor()), true);
      appendNumberField(json, 2, "densityOffset", Double.toString(noiseSettings.densityOffset()), true);
      appendNumberField(json, 2, "useSimplexSurfaceNoise", Boolean.toString(noiseSettings.useSimplexSurfaceNoise()), true);
      appendNumberField(json, 2, "randomDensityOffset", Boolean.toString(noiseSettings.randomDensityOffset()), true);
      appendNumberField(json, 2, "islandNoiseOverride", Boolean.toString(noiseSettings.islandNoiseOverride()), true);
      appendNumberField(json, 2, "isAmplified", Boolean.toString(noiseSettings.isAmplified()), false);
      indent(json, 1);
      json.append("}");
   }

   private static void appendNoiseSamplerSampleSet(
      StringBuilder json,
      int indentLevel,
      BiomePatternSpec pattern,
      long seed,
      NoiseSettings noiseSettings,
      int cellWidth,
      int cellHeight,
      int cellCountY,
      int biomeY,
      int minCellY,
      int columnValueCount,
      int[] sampleX,
      int[] sampleZ,
      int sampleCount,
      boolean trailingComma
   ) {
      ResolvedBiomePattern resolvedPattern = resolveBiomePattern(pattern);
      NoiseSampler sampler = createOverworldNoiseSampler(seed, new RepeatingPatternBiomeSource(resolvedPattern.biomes), noiseSettings, cellWidth, cellHeight, cellCountY);
      indent(json, indentLevel);
      json.append('"');
      json.append(escapeJson(pattern.name));
      json.append("\": {\n");
      appendBiomePattern(json, indentLevel + 1, resolvedPattern, true);
      appendNoiseSamplerColumns(
         json,
         indentLevel + 1,
         sampler,
         noiseSettings,
         biomeY,
         minCellY,
         cellCountY,
         columnValueCount,
         sampleX,
         sampleZ,
         sampleCount,
         false
      );
      indent(json, indentLevel);
      json.append('}');
      if (trailingComma) {
         json.append(',');
      }

      json.append('\n');
   }

   private static void appendBiomePattern(StringBuilder json, int indentLevel, ResolvedBiomePattern pattern, boolean trailingComma) {
      indent(json, indentLevel);
      json.append("\"biomePattern\": {\n");
      appendField(json, indentLevel + 1, "gridOrder", "x-major,z-minor", true);
      appendNumberField(json, indentLevel + 1, "sampleCount", Integer.toString(pattern.keys.length), true);
      indent(json, indentLevel + 1);
      json.append("\"x\": ");
      appendIntArray(json, NOISE_SAMPLER_PATTERN_AXIS);
      json.append(",\n");
      indent(json, indentLevel + 1);
      json.append("\"z\": ");
      appendIntArray(json, NOISE_SAMPLER_PATTERN_AXIS);
      json.append(",\n");
      indent(json, indentLevel + 1);
      json.append("\"keys\": ");
      appendStringArray(json, pattern.keys);
      json.append(",\n");
      indent(json, indentLevel + 1);
      json.append("\"depths\": ");
      appendDoubleArray(json, pattern.depths);
      json.append(",\n");
      indent(json, indentLevel + 1);
      json.append("\"scales\": ");
      appendDoubleArray(json, pattern.scales);
      json.append('\n');
      indent(json, indentLevel);
      json.append('}');
      if (trailingComma) {
         json.append(',');
      }

      json.append('\n');
   }

   private static void appendNoiseSamplerColumns(
      StringBuilder json,
      int indentLevel,
      NoiseSampler sampler,
      NoiseSettings noiseSettings,
      int biomeY,
      int minCellY,
      int cellCountY,
      int columnValueCount,
      int[] sampleX,
      int[] sampleZ,
      int sampleCount,
      boolean trailingComma
   ) {
      indent(json, indentLevel);
      json.append("\"columns\": {\n");
      appendField(
         json,
         indentLevel + 1,
         "noiseMethod",
         "fillNoiseColumn(noiseValues,cellX,cellZ,noiseSettings,biomeY,minCellY,cellCountY)",
         true
      );
      appendField(json, indentLevel + 1, "gridOrder", "x-major,z-minor,y-minor", true);
      appendNumberField(json, indentLevel + 1, "columnValueCount", Integer.toString(columnValueCount), true);
      appendNumberField(json, indentLevel + 1, "sampleCount", Integer.toString(sampleCount), true);
      indent(json, indentLevel + 1);
      json.append("\"x\": ");
      appendIntArray(json, sampleX);
      json.append(",\n");
      indent(json, indentLevel + 1);
      json.append("\"z\": ");
      appendIntArray(json, sampleZ);
      json.append(",\n");
      indent(json, indentLevel + 1);
      json.append("\"values\": ");
      appendNoiseSamplerColumnValueArray(json, sampler, noiseSettings, biomeY, minCellY, cellCountY, columnValueCount, sampleX, sampleZ);
      json.append('\n');
      indent(json, indentLevel);
      json.append('}');
      if (trailingComma) {
         json.append(',');
      }

      json.append('\n');
   }

   private static void appendNoiseSamplerColumnValueArray(
      StringBuilder json,
      NoiseSampler sampler,
      NoiseSettings noiseSettings,
      int biomeY,
      int minCellY,
      int cellCountY,
      int columnValueCount,
      int[] sampleX,
      int[] sampleZ
   ) {
      double[] column = new double[columnValueCount];
      json.append('[');
      boolean first = true;

      for (int cellX : sampleX) {
         for (int cellZ : sampleZ) {
            sampler.fillNoiseColumn(column, cellX, cellZ, noiseSettings, biomeY, minCellY, cellCountY);

            for (int index = 0; index < columnValueCount; index++) {
               if (!first) {
                  json.append(',');
               }

               first = false;
               json.append(Double.toString(column[index]));
            }
         }
      }

      json.append(']');
   }

   private static void appendBlendedSampleSet(
      StringBuilder json,
      int indentLevel,
      BlendedNoiseSampleParameters params,
      IntegerSampleGrid sampleGrid,
      int sampleCount,
      BlendedNoise noise,
      PerlinNoise mainNoise,
      boolean trailingComma
   ) {
      indent(json, indentLevel);
      json.append('"');
      json.append(escapeJson(params.name));
      json.append("\": {\n");
      appendField(json, indentLevel + 1, "gridOrder", sampleGrid.gridOrder, true);
      appendNumberField(json, indentLevel + 1, "sampleCount", Integer.toString(sampleCount), true);
      indent(json, indentLevel + 1);
      json.append("\"settingsKeys\": ");
      appendStringArray(json, params.settingsKeys);
      json.append(",\n");
      indent(json, indentLevel + 1);
      json.append("\"parameters\": {\n");
      appendNumberField(json, indentLevel + 2, "limitHorizontalScale", Double.toString(params.limitHorizontalScale), true);
      appendNumberField(json, indentLevel + 2, "limitVerticalScale", Double.toString(params.limitVerticalScale), true);
      appendNumberField(json, indentLevel + 2, "mainHorizontalScale", Double.toString(params.mainHorizontalScale), true);
      appendNumberField(json, indentLevel + 2, "mainVerticalScale", Double.toString(params.mainVerticalScale), false);
      indent(json, indentLevel + 1);
      json.append("},\n");
      indent(json, indentLevel + 1);
      json.append("\"x\": ");
      appendIntArray(json, sampleGrid.x);
      json.append(",\n");
      indent(json, indentLevel + 1);
      json.append("\"y\": ");
      appendIntArray(json, sampleGrid.y);
      json.append(",\n");
      indent(json, indentLevel + 1);
      json.append("\"z\": ");
      appendIntArray(json, sampleGrid.z);
      json.append(",\n");
      indent(json, indentLevel + 1);
      json.append("\"values\": ");
      BlendFactorSummary summary = appendBlendedNoiseValueArray(json, sampleGrid, noise, mainNoise, params);
      json.append(",\n");
      indent(json, indentLevel + 1);
      json.append("\"blendFactorRange\": {\n");
      appendNumberField(json, indentLevel + 2, "min", Double.toString(summary.min), true);
      appendNumberField(json, indentLevel + 2, "max", Double.toString(summary.max), false);
      indent(json, indentLevel + 1);
      json.append("},\n");
      indent(json, indentLevel + 1);
      json.append("\"blendRegionCounts\": {\n");
      appendNumberField(json, indentLevel + 2, "belowOrEqualZero", Integer.toString(summary.belowOrEqualZero), true);
      appendNumberField(json, indentLevel + 2, "interior", Integer.toString(summary.interior), true);
      appendNumberField(json, indentLevel + 2, "aboveOrEqualOne", Integer.toString(summary.aboveOrEqualOne), false);
      indent(json, indentLevel + 1);
      json.append("}\n");
      indent(json, indentLevel);
      json.append('}');
      if (trailingComma) {
         json.append(',');
      }

      json.append('\n');
   }

   private static double[] createAxis(double start, double step, int count) {
      double[] values = new double[count];

      for (int index = 0; index < count; index++) {
         values[index] = start + step * index;
      }

      return values;
   }

   private static int[] createIntAxis(int start, int step, int count) {
      int[] values = new int[count];

      for (int index = 0; index < count; index++) {
         values[index] = start + step * index;
      }

      return values;
   }

   private static BlendedNoiseSampleParameters blendedNoiseSampleParameters(
      String name,
      String[] settingsKeys,
      double xzScale,
      double yScale,
      double xzFactor,
      double yFactor
   ) {
      double limitHorizontalScale = 684.412 * xzScale;
      double limitVerticalScale = 684.412 * yScale;
      return new BlendedNoiseSampleParameters(
         name,
         settingsKeys,
         limitHorizontalScale,
         limitVerticalScale,
         limitHorizontalScale / xzFactor,
         limitVerticalScale / yFactor
      );
   }

   private static int[] parseOctaves(String value) {
      String[] parts = value.split(",");
      if (parts.length == 0) {
         throw new IllegalArgumentException("octaves must not be empty");
      }

      int[] octaves = new int[parts.length];
      for (int index = 0; index < parts.length; index++) {
         String part = parts[index].trim();
         if (part.isEmpty()) {
            throw new IllegalArgumentException("empty octave entry");
         }

         try {
            octaves[index] = Integer.parseInt(part);
         } catch (NumberFormatException error) {
            throw new IllegalArgumentException("invalid octave '" + part + "'");
         }
      }

      java.util.Arrays.sort(octaves);
      int uniqueCount = 0;
      for (int octave : octaves) {
         if (uniqueCount == 0 || octaves[uniqueCount - 1] != octave) {
            octaves[uniqueCount++] = octave;
         }
      }

      return java.util.Arrays.copyOf(octaves, uniqueCount);
   }

   private static double[] parseAmplitudes(String value) {
      String[] parts = value.split(",", -1);
      double[] amplitudes = new double[parts.length];

      for (int index = 0; index < parts.length; index++) {
         String part = parts[index].trim();
         if (part.isEmpty()) {
            throw new IllegalArgumentException("empty amplitude entry");
         }

         try {
            amplitudes[index] = Double.parseDouble(part);
         } catch (NumberFormatException error) {
            throw new IllegalArgumentException("invalid amplitude '" + part + "'");
         }

         if (!Double.isFinite(amplitudes[index])) {
            throw new IllegalArgumentException("invalid amplitude '" + part + "'");
         }
      }

      return amplitudes;
   }

   private static int[] requireIntegerAxis(double[] values, String axisName) {
      int[] axis = new int[values.length];

      for (int index = 0; index < values.length; index++) {
         double value = values[index];
         if (value != Math.rint(value) || value < Integer.MIN_VALUE || value > Integer.MAX_VALUE) {
            throw new IllegalArgumentException("NoiseSampler sample axis '" + axisName + "' must contain integers");
         }

         axis[index] = (int)value;
      }

      return axis;
   }

   private static NoiseSampler createOverworldNoiseSampler(
      long seed,
      BiomeSource biomeSource,
      NoiseSettings noiseSettings,
      int cellWidth,
      int cellHeight,
      int cellCountY
   ) {
      WorldgenRandom random = new WorldgenRandom(seed);
      BlendedNoise blendedNoise = new BlendedNoise(random);
      random.consumeCount(2620);
      PerlinNoise depthNoise = new PerlinNoise(random, toIntegerList(BLENDED_LIMIT_OCTAVES));
      return new NoiseSampler(biomeSource, cellWidth, cellHeight, cellCountY, noiseSettings, blendedNoise, null, depthNoise, NoiseModifier.PASSTHROUGH);
   }

   private static ResolvedBiomePattern resolveBiomePattern(BiomePatternSpec pattern) {
      String[] keys = new String[pattern.biomeKeys.length];
      double[] depths = new double[pattern.biomeKeys.length];
      double[] scales = new double[pattern.biomeKeys.length];
      Biome[] biomes = new Biome[pattern.biomeKeys.length];

      for (int index = 0; index < pattern.biomeKeys.length; index++) {
         ResourceLocation key = new ResourceLocation(pattern.biomeKeys[index]);
         Biome biome = BuiltinRegistries.BIOME.get(key);
         if (biome == null) {
            throw new IllegalArgumentException("unknown biome key '" + key + "'");
         }

         biomes[index] = biome;
         keys[index] = BuiltinRegistries.BIOME.getKey(biome).toString();
         depths[index] = biome.getDepth();
         scales[index] = biome.getScale();
      }

      return new ResolvedBiomePattern(keys, depths, scales, biomes);
   }

   private static List<Integer> toIntegerList(int[] values) {
      List<Integer> result = new ArrayList<>(values.length);
      for (int value : values) {
         result.add(value);
      }

      return result;
   }

   private static SampleGrid loadSampleGrid(String samplePath) throws java.io.IOException {
      String json = Files.readString(Path.of(samplePath), StandardCharsets.UTF_8);
      SampleGrid sampleGrid = GSON.fromJson(json, SampleGrid.class);
      if (sampleGrid == null) {
         throw new IllegalArgumentException("failed to parse sample grid '" + samplePath + "'");
      }

      validateSampleGrid(sampleGrid, samplePath);
      return sampleGrid;
   }

   private static SampleGrid2D loadSampleGrid2D(String samplePath) throws java.io.IOException {
      String json = Files.readString(Path.of(samplePath), StandardCharsets.UTF_8);
      SampleGrid2D sampleGrid = GSON.fromJson(json, SampleGrid2D.class);
      if (sampleGrid == null) {
         throw new IllegalArgumentException("failed to parse sample grid '" + samplePath + "'");
      }

      validateSampleGrid2D(sampleGrid, samplePath);
      return sampleGrid;
   }

   private static IntegerSampleGrid loadIntegerSampleGrid(String samplePath) throws java.io.IOException {
      String json = Files.readString(Path.of(samplePath), StandardCharsets.UTF_8);
      IntegerSampleGrid sampleGrid = GSON.fromJson(json, IntegerSampleGrid.class);
      if (sampleGrid == null) {
         throw new IllegalArgumentException("failed to parse sample grid '" + samplePath + "'");
      }

      validateIntegerSampleGrid(sampleGrid, samplePath);
      return sampleGrid;
   }

   private static SampleGrid defaultNoiseSampleGrid() {
      return new SampleGrid("x-major,y-major,z-minor", NOISE_X_COORDS, NOISE_Y_COORDS, NOISE_Z_COORDS);
   }

   private static void validateSampleGrid(SampleGrid sampleGrid, String samplePath) {
      if (sampleGrid.gridOrder == null || sampleGrid.gridOrder.isEmpty()) {
         throw new IllegalArgumentException("sample grid '" + samplePath + "' is missing gridOrder");
      }

      if (sampleGrid.x == null || sampleGrid.x.length == 0 || sampleGrid.y == null || sampleGrid.y.length == 0 || sampleGrid.z == null || sampleGrid.z.length == 0) {
         throw new IllegalArgumentException("sample grid '" + samplePath + "' must provide non-empty x/y/z arrays");
      }
   }

   private static void validateSampleGrid2D(SampleGrid2D sampleGrid, String samplePath) {
      if (sampleGrid.gridOrder == null || sampleGrid.gridOrder.isEmpty()) {
         throw new IllegalArgumentException("sample grid '" + samplePath + "' is missing gridOrder");
      }

      if (sampleGrid.x == null || sampleGrid.x.length == 0 || sampleGrid.z == null || sampleGrid.z.length == 0) {
         throw new IllegalArgumentException("sample grid '" + samplePath + "' must provide non-empty x/z arrays");
      }
   }

   private static void validateIntegerSampleGrid(IntegerSampleGrid sampleGrid, String samplePath) {
      if (sampleGrid.gridOrder == null || sampleGrid.gridOrder.isEmpty()) {
         throw new IllegalArgumentException("sample grid '" + samplePath + "' is missing gridOrder");
      }

      if (sampleGrid.x == null || sampleGrid.x.length == 0 || sampleGrid.y == null || sampleGrid.y.length == 0 || sampleGrid.z == null || sampleGrid.z.length == 0) {
         throw new IllegalArgumentException("sample grid '" + samplePath + "' must provide non-empty x/y/z arrays");
      }
   }

   private static double computeBlendedNoiseFactor(int x, int y, int z, BlendedNoiseSampleParameters params, PerlinNoise mainNoise) {
      double value = 0.0;
      double inputFactor = 1.0;

      for (int octave = 0; octave < 8; octave++) {
         ImprovedNoise noise = mainNoise.getOctaveNoise(octave);
         if (noise != null) {
            value += noise.noise(
                  PerlinNoise.wrap(x * params.mainHorizontalScale * inputFactor),
                  PerlinNoise.wrap(y * params.mainVerticalScale * inputFactor),
                  PerlinNoise.wrap(z * params.mainHorizontalScale * inputFactor),
                  params.mainVerticalScale * inputFactor,
                  y * params.mainVerticalScale * inputFactor
               )
               / inputFactor;
         }

         inputFactor /= 2.0;
      }

      return (value / 10.0 + 1.0) / 2.0;
   }

   private static void appendField(StringBuilder json, int indentLevel, String name, String value, boolean trailingComma) {
      indent(json, indentLevel);
      json.append('"');
      json.append(escapeJson(name));
      json.append("\": \"");
      json.append(escapeJson(value));
      json.append('"');
      if (trailingComma) {
         json.append(',');
      }

      json.append('\n');
   }

   private static void appendNumberField(StringBuilder json, int indentLevel, String name, String value, boolean trailingComma) {
      indent(json, indentLevel);
      json.append('"');
      json.append(escapeJson(name));
      json.append("\": ");
      json.append(value);
      if (trailingComma) {
         json.append(',');
      }

      json.append('\n');
   }

   private static void indent(StringBuilder json, int indentLevel) {
      for (int index = 0; index < indentLevel; index++) {
         json.append("  ");
      }
   }

   private static String escapeJson(String value) {
      StringBuilder escaped = new StringBuilder(value.length() + 8);

      for (int index = 0; index < value.length(); index++) {
         char current = value.charAt(index);
         switch (current) {
            case '\\':
               escaped.append("\\\\");
               break;
            case '"':
               escaped.append("\\\"");
               break;
            case '\b':
               escaped.append("\\b");
               break;
            case '\f':
               escaped.append("\\f");
               break;
            case '\n':
               escaped.append("\\n");
               break;
            case '\r':
               escaped.append("\\r");
               break;
            case '\t':
               escaped.append("\\t");
               break;
            default:
               if (current < 0x20) {
                  escaped.append(String.format("\\u%04x", (int)current));
               } else {
                  escaped.append(current);
               }
               break;
         }
      }

      return escaped.toString();
   }

   private static void printUsage() {
      System.err.println("usage:");
      System.err.println("  oracle-dumper prng --seed <long> --count <positive-int>");
      System.err.println("  oracle-dumper biome --class OverworldBiomeSource --seed <long> --samples2d <path> [--legacy-biome-init-layer <true|false>] [--large-biomes <true|false>]");
      System.err.println("  oracle-dumper noise --seed <long>");
      System.err.println("  oracle-dumper noise --class NoiseSampler --seed <long> --preset overworld --samples2d <path>");
      System.err.println("  oracle-dumper terrain-chunk --seed <long> --chunk-x <int> --chunk-z <int>");
      System.err.println("  oracle-dumper surface-chunk --seed <long> --chunk-x <int> --chunk-z <int>");
      System.err.println("  oracle-dumper carved-chunk --seed <long> --chunk-x <int> --chunk-z <int>");
      System.err.println("  oracle-dumper liquid-carved-chunk --seed <long> --chunk-x <int> --chunk-z <int>");
      System.err.println("  oracle-dumper feature-order-trace --seed <long> --chunk-x <int> --chunk-z <int> [--generate-structures <true|false>]");
      System.err.println("  oracle-dumper scheduler-trace --seed <long> --chunk-x <int> --chunk-z <int> [--scenario spawn_bootstrap] [--target-radius <int>] [--record-radius <int>] [--stop-status features|full] [--generate-structures <true|false>] [--dump-chunks <true|false>] [--dump-only-target-chunk <true|false>] [--probe-blocks <x,y,z;...>] [--probe-target-tree-blocks <true|false>] [--probe-tree-candidates <true|false>] [--tree-probe-center-x <int>] [--tree-probe-center-z <int>] [--tree-probe-step-index <int>] [--tree-probe-feature-index <int>] [--probe-ore-placements <true|false>] [--ore-probe-center-x <int>] [--ore-probe-center-z <int>] [--ore-probe-step-index <int>] [--ore-probe-feature-index <int>]");
      System.err.println("  oracle-dumper visgraph --seed <long>");
      System.err.println("  oracle-dumper synthetic-light --seed <long>");
      System.err.println("  oracle-dumper noise --class NormalNoise --seed <long> --first-octave <int> --amplitudes <csv> --samples <path>");
      System.err.println("  oracle-dumper noise --class PerlinNoise --seed <long> --octaves <csv> --samples <path>");
      System.err.println("  oracle-dumper noise --class PerlinSimplexNoise --seed <long> --octaves <csv> --samples2d <path>");
      System.err.println("  oracle-dumper noise --class SimplexNoise --seed <long> --samples2d <path> --samples3d <path>");
      System.err.println("  oracle-dumper noise --class BlendedNoise --seed <long> --samples <path>");
      System.err.println("dumps Minecraft " + MINECRAFT_VERSION + " oracle fixtures as JSON");
      System.err.println("note: 1.17.1 uses SimpleRandomSource; later mappings rename this legacy LCG to LegacyRandomSource");
   }

   @FunctionalInterface
   private interface NoiseValueSampler3D {
      double sample(double x, double y, double z);
   }

   @FunctionalInterface
   private interface NoiseValueSampler2D {
      double sample(double x, double z);
   }

    private static final class BiomePatternSpec {
      final String name;
      final String[] biomeKeys;

      BiomePatternSpec(String name, String[] biomeKeys) {
         this.name = name;
         this.biomeKeys = biomeKeys;
      }
   }

   private static final class ResolvedBiomePattern {
      final String[] keys;
      final double[] depths;
      final double[] scales;
      final Biome[] biomes;

      ResolvedBiomePattern(String[] keys, double[] depths, double[] scales, Biome[] biomes) {
         this.keys = keys;
         this.depths = depths;
         this.scales = scales;
         this.biomes = biomes;
      }
   }

   private static final class RepeatingPatternBiomeSource extends BiomeSource {
      private final Biome[] pattern;

      RepeatingPatternBiomeSource(Biome[] pattern) {
         super(java.util.Arrays.asList(pattern));
         this.pattern = pattern;
      }

      @Override
      protected com.mojang.serialization.Codec<? extends BiomeSource> codec() {
         throw new UnsupportedOperationException("oracle-only biome source");
      }

      @Override
      public BiomeSource withSeed(long seed) {
         return this;
      }

      @Override
      public Biome getNoiseBiome(int x, int y, int z) {
         int wrappedX = Math.floorMod(x, NOISE_SAMPLER_PATTERN_AXIS.length);
         int wrappedZ = Math.floorMod(z, NOISE_SAMPLER_PATTERN_AXIS.length);
         return this.pattern[wrappedX * NOISE_SAMPLER_PATTERN_AXIS.length + wrappedZ];
      }
   }

   private static final class SampleGrid {
      String gridOrder;
      double[] x;
      double[] y;
      double[] z;

      SampleGrid() {
      }

      SampleGrid(String gridOrder, double[] x, double[] y, double[] z) {
         this.gridOrder = gridOrder;
         this.x = x;
         this.y = y;
         this.z = z;
      }
   }

   private static final class SampleGrid2D {
      String gridOrder;
      double[] x;
      double[] z;

      SampleGrid2D() {
      }
   }

   private static final class IntegerSampleGrid {
      String gridOrder;
      int[] x;
      int[] y;
      int[] z;

      IntegerSampleGrid() {
      }
   }

   private static final class BlendedNoiseSampleParameters {
      final String name;
      final String[] settingsKeys;
      final double limitHorizontalScale;
      final double limitVerticalScale;
      final double mainHorizontalScale;
      final double mainVerticalScale;

      BlendedNoiseSampleParameters(
         String name,
         String[] settingsKeys,
         double limitHorizontalScale,
         double limitVerticalScale,
         double mainHorizontalScale,
         double mainVerticalScale
      ) {
         this.name = name;
         this.settingsKeys = settingsKeys;
         this.limitHorizontalScale = limitHorizontalScale;
         this.limitVerticalScale = limitVerticalScale;
         this.mainHorizontalScale = mainHorizontalScale;
         this.mainVerticalScale = mainVerticalScale;
      }
   }

   private static final class BlendFactorSummary {
      double min = Double.POSITIVE_INFINITY;
      double max = Double.NEGATIVE_INFINITY;
      int belowOrEqualZero;
      int interior;
      int aboveOrEqualOne;

      void record(double blendFactor) {
         this.min = Math.min(this.min, blendFactor);
         this.max = Math.max(this.max, blendFactor);
         if (blendFactor <= 0.0) {
            this.belowOrEqualZero++;
         } else if (blendFactor >= 1.0) {
            this.aboveOrEqualOne++;
         } else {
            this.interior++;
         }
      }
   }
}
