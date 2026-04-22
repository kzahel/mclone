import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.world.level.levelgen.SimpleRandomSource;
import net.minecraft.world.level.levelgen.synth.ImprovedNoise;

public final class OracleDumper {
   private static final String MINECRAFT_VERSION = "1.17.1";
   private static final String RANDOM_SOURCE_CLASS = "net.minecraft.world.level.levelgen.SimpleRandomSource";
   private static final String RANDOM_SOURCE_ALIAS = "LegacyRandomSource";
   private static final String IMPROVED_NOISE_CLASS = "net.minecraft.world.level.levelgen.synth.ImprovedNoise";
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

   private static void run(String[] args) {
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
            json = dumpNoise(seed);
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

   private static String dumpNoise(long seed) {
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
      json.append('[');
      boolean first = true;

      for (double x : NOISE_X_COORDS) {
         for (double y : NOISE_Y_COORDS) {
            for (double z : NOISE_Z_COORDS) {
               if (!first) {
                  json.append(',');
               }

               first = false;
               json.append(Double.toString(noise.noise(x, y, z)));
            }
         }
      }

      json.append(']');
   }

   private static double[] createAxis(double start, double step, int count) {
      double[] values = new double[count];

      for (int index = 0; index < count; index++) {
         values[index] = start + step * index;
      }

      return values;
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
      System.err.println("dumps Minecraft " + MINECRAFT_VERSION + " oracle fixtures as JSON");
      System.err.println("note: 1.17.1 uses SimpleRandomSource; later mappings rename this legacy LCG to LegacyRandomSource");
   }
}
