import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.world.level.levelgen.SimpleRandomSource;

public final class OracleDumper {
   private static final String MINECRAFT_VERSION = "1.17.1";
   private static final String RANDOM_SOURCE_CLASS = "net.minecraft.world.level.levelgen.SimpleRandomSource";
   private static final String RANDOM_SOURCE_ALIAS = "LegacyRandomSource";

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
      int count = parseCount(requireOption(options, "count"));

      String json;
      switch (module) {
         case "prng":
            json = dumpPrng(seed, count);
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
      System.err.println("usage: oracle-dumper prng --seed <long> --count <positive-int>");
      System.err.println("dumps Minecraft " + MINECRAFT_VERSION + " PRNG fixtures as JSON");
      System.err.println("note: 1.17.1 uses SimpleRandomSource; later mappings rename this legacy LCG to LegacyRandomSource");
   }
}
