import com.google.gson.Gson;
import java.util.LinkedHashMap;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import net.minecraft.world.level.levelgen.SimpleRandomSource;
import net.minecraft.world.level.levelgen.synth.ImprovedNoise;
import net.minecraft.world.level.levelgen.synth.PerlinNoise;
import net.minecraft.world.level.levelgen.synth.SimplexNoise;

public final class OracleDumper {
   private static final String MINECRAFT_VERSION = "1.17.1";
   private static final String RANDOM_SOURCE_CLASS = "net.minecraft.world.level.levelgen.SimpleRandomSource";
   private static final String RANDOM_SOURCE_ALIAS = "LegacyRandomSource";
   private static final String IMPROVED_NOISE_CLASS = ImprovedNoise.class.getName();
   private static final String PERLIN_NOISE_CLASS = PerlinNoise.class.getName();
   private static final String SIMPLEX_NOISE_CLASS = SimplexNoise.class.getName();
   private static final Gson GSON = new Gson();
   private static final double[] NOISE_X_COORDS = createAxis(-2.5, 0.5, 11);
   private static final double[] NOISE_Y_COORDS = createAxis(-1.875, 0.375, 11);
   private static final double[] NOISE_Z_COORDS = createAxis(-3.125, 0.625, 11);

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
         case "noise":
            json = dumpNoise(seed, options);
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
         case "PerlinNoise":
            return dumpPerlinNoise(seed, parseOctaves(requireOption(options, "octaves")), loadSampleGrid(requireOption(options, "samples")));
         case "SimplexNoise":
            return dumpSimplexNoise(seed, loadSampleGrid2D(requireOption(options, "samples2d")), loadSampleGrid(requireOption(options, "samples3d")));
         default:
            throw new IllegalArgumentException("unsupported noise class '" + className + "'");
      }
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

   private static void appendNoiseValueArray(StringBuilder json, ImprovedNoise noise) {
      appendNoiseValueArray(json, defaultNoiseSampleGrid(), noise::noise);
   }

   private static void appendNoiseValueArray(StringBuilder json, SampleGrid sampleGrid, NoiseSampler noise) {
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

   private static void appendNoiseValueArray(StringBuilder json, SampleGrid2D sampleGrid, NoiseSampler2D noise) {
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

   private static void appendSampleSet2D(StringBuilder json, String name, String noiseMethod, SampleGrid2D sampleGrid, int sampleCount, NoiseSampler2D noise, boolean trailingComma) {
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

   private static void appendSampleSet3D(StringBuilder json, String name, String noiseMethod, SampleGrid sampleGrid, int sampleCount, NoiseSampler noise, boolean trailingComma) {
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

   private static double[] createAxis(double start, double step, int count) {
      double[] values = new double[count];

      for (int index = 0; index < count; index++) {
         values[index] = start + step * index;
      }

      return values;
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
      System.err.println("  oracle-dumper noise --seed <long>");
      System.err.println("  oracle-dumper noise --class PerlinNoise --seed <long> --octaves <csv> --samples <path>");
      System.err.println("  oracle-dumper noise --class SimplexNoise --seed <long> --samples2d <path> --samples3d <path>");
      System.err.println("dumps Minecraft " + MINECRAFT_VERSION + " oracle fixtures as JSON");
      System.err.println("note: 1.17.1 uses SimpleRandomSource; later mappings rename this legacy LCG to LegacyRandomSource");
   }

   @FunctionalInterface
   private interface NoiseSampler {
      double sample(double x, double y, double z);
   }

   @FunctionalInterface
   private interface NoiseSampler2D {
      double sample(double x, double z);
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
}
