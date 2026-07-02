import java.nio.charset.StandardCharsets;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

public final class VanillaClientLauncher {
   private static final String DEFAULT_USERNAME = "McloneOracle";
   private static final String DEFAULT_ACCESS_TOKEN = "0";
   private static final String DEFAULT_VERSION = "mclone-vanilla-1.17.1";
   private static final String DEFAULT_USER_TYPE = "legacy";
   private static final String DEFAULT_VERSION_TYPE = "release";
   private static final int DEFAULT_WIDTH = 854;
   private static final int DEFAULT_HEIGHT = 480;

   private VanillaClientLauncher() {
   }

   public static void main(String[] args) {
      try {
         Options options = parseOptions(args);
         List<String> minecraftArgs = buildMinecraftArgs(options);

         if (options.printArgs) {
            System.out.println(toJsonArray(minecraftArgs));
         }

         if (options.launch) {
            if (options.screenshotPath != null) {
               VanillaClientScreenshotHarness.start(options.screenshotConfig());
            }
            net.minecraft.client.main.Main.main(minecraftArgs.toArray(new String[0]));
         }
      } catch (IllegalArgumentException error) {
         System.err.println("error: " + error.getMessage());
         System.err.println();
         printUsage();
         System.exit(1);
      } catch (Throwable error) {
         error.printStackTrace(System.err);
         System.exit(1);
      }
   }

   private static Options parseOptions(String[] args) {
      Options options = new Options();

      for (int index = 0; index < args.length; index++) {
         String argument = args[index];
         switch (argument) {
            case "--mclone-launch":
               options.launch = true;
               break;
            case "--mclone-print-args":
               options.printArgs = true;
               break;
            case "--mclone-game-dir":
               options.gameDir = requireValue(args, ++index, argument);
               break;
            case "--mclone-assets-dir":
               options.assetsDir = requireValue(args, ++index, argument);
               break;
            case "--mclone-asset-index":
               options.assetIndex = requireValue(args, ++index, argument);
               break;
            case "--mclone-username":
               options.username = requireValue(args, ++index, argument);
               break;
            case "--mclone-uuid":
               options.uuid = requireValue(args, ++index, argument);
               break;
            case "--mclone-access-token":
               options.accessToken = requireValue(args, ++index, argument);
               break;
            case "--mclone-version":
               options.version = requireValue(args, ++index, argument);
               break;
            case "--mclone-user-type":
               options.userType = requireValue(args, ++index, argument);
               break;
            case "--mclone-version-type":
               options.versionType = requireValue(args, ++index, argument);
               break;
            case "--mclone-width":
               options.width = parsePositiveInt(requireValue(args, ++index, argument), "width");
               break;
            case "--mclone-height":
               options.height = parsePositiveInt(requireValue(args, ++index, argument), "height");
               break;
            case "--mclone-screenshot":
               options.screenshotPath = requireValue(args, ++index, argument);
               options.launch = true;
               break;
            case "--mclone-seed":
               options.seed = parseLong(requireValue(args, ++index, argument), "seed");
               break;
            case "--mclone-world-name":
               options.worldName = requireValue(args, ++index, argument);
               break;
            case "--mclone-camera":
               options.setCamera(requireValue(args, ++index, argument));
               break;
            case "--mclone-day-time":
               options.dayTime = parseLong(requireValue(args, ++index, argument), "day time");
               break;
            case "--mclone-settle-frames":
               options.settleFrames = parsePositiveInt(requireValue(args, ++index, argument), "settle frames");
               break;
            case "--mclone-timeout-seconds":
               options.timeoutSeconds = parsePositiveInt(requireValue(args, ++index, argument), "timeout seconds");
               break;
            case "--mclone-render-distance":
               options.renderDistance = parseRenderDistance(requireValue(args, ++index, argument));
               break;
            case "--mclone-show-gui":
               options.hideGui = false;
               break;
            case "--mclone-no-structures":
               options.generateStructures = false;
               break;
            case "--mclone-disable-multiplayer":
               options.disableMultiplayer = true;
               break;
            case "--mclone-disable-chat":
               options.disableChat = true;
               break;
            default:
               throw new IllegalArgumentException("unsupported option '" + argument + "'");
         }
      }

      if (options.gameDir == null || options.gameDir.isEmpty()) {
         throw new IllegalArgumentException("--mclone-game-dir is required");
      }
      if (options.assetsDir == null || options.assetsDir.isEmpty()) {
         throw new IllegalArgumentException("--mclone-assets-dir is required");
      }
      if (options.assetIndex == null || options.assetIndex.isEmpty()) {
         throw new IllegalArgumentException("--mclone-asset-index is required");
      }
      if (options.username == null || options.username.isEmpty()) {
         options.username = DEFAULT_USERNAME;
      }
      if (options.uuid == null || options.uuid.isEmpty()) {
         options.uuid = offlineUuid(options.username);
      }
      if (options.accessToken == null) {
         options.accessToken = DEFAULT_ACCESS_TOKEN;
      }
      if (options.version == null || options.version.isEmpty()) {
         options.version = DEFAULT_VERSION;
      }
      if (options.userType == null || options.userType.isEmpty()) {
         options.userType = DEFAULT_USER_TYPE;
      }
      if (options.versionType == null || options.versionType.isEmpty()) {
         options.versionType = DEFAULT_VERSION_TYPE;
      }

      if (options.screenshotPath != null && options.screenshotPath.isEmpty()) {
         throw new IllegalArgumentException("--mclone-screenshot requires a value");
      }

      return options;
   }

   private static List<String> buildMinecraftArgs(Options options) {
      ArrayList<String> args = new ArrayList<>();
      args.add("--username");
      args.add(options.username);
      args.add("--version");
      args.add(options.version);
      args.add("--gameDir");
      args.add(options.gameDir);
      args.add("--assetsDir");
      args.add(options.assetsDir);
      args.add("--assetIndex");
      args.add(options.assetIndex);
      args.add("--uuid");
      args.add(options.uuid);
      args.add("--accessToken");
      args.add(options.accessToken);
      args.add("--userType");
      args.add(options.userType);
      args.add("--versionType");
      args.add(options.versionType);
      args.add("--width");
      args.add(Integer.toString(options.width));
      args.add("--height");
      args.add(Integer.toString(options.height));
      if (options.disableMultiplayer) {
         args.add("--disableMultiplayer");
      }
      if (options.disableChat) {
         args.add("--disableChat");
      }
      return args;
   }

   private static String requireValue(String[] args, int index, String option) {
      if (index >= args.length) {
         throw new IllegalArgumentException(option + " requires a value");
      }
      String value = args[index];
      if (value.startsWith("--mclone-")) {
         throw new IllegalArgumentException(option + " requires a value");
      }
      return value;
   }

   private static int parsePositiveInt(String value, String name) {
      try {
         int parsed = Integer.parseInt(value);
         if (parsed <= 0) {
            throw new IllegalArgumentException(name + " must be positive");
         }
         return parsed;
      } catch (NumberFormatException error) {
         throw new IllegalArgumentException(name + " must be an integer");
      }
   }

   private static long parseLong(String value, String name) {
      try {
         return Long.parseLong(value);
      } catch (NumberFormatException error) {
         throw new IllegalArgumentException(name + " must be an integer");
      }
   }

   private static double parseDouble(String value, String name) {
      try {
         return Double.parseDouble(value);
      } catch (NumberFormatException error) {
         throw new IllegalArgumentException(name + " must be numeric");
      }
   }

   private static String offlineUuid(String username) {
      return UUID.nameUUIDFromBytes(("OfflinePlayer:" + username).getBytes(StandardCharsets.UTF_8)).toString();
   }

   private static String toJsonArray(List<String> values) {
      StringBuilder builder = new StringBuilder();
      builder.append('[');
      for (int index = 0; index < values.size(); index++) {
         if (index > 0) {
            builder.append(',');
         }
         builder.append('"');
         appendJsonString(builder, values.get(index));
         builder.append('"');
      }
      builder.append(']');
      return builder.toString();
   }

   private static void appendJsonString(StringBuilder builder, String value) {
      for (int index = 0; index < value.length(); index++) {
         char character = value.charAt(index);
         switch (character) {
            case '"':
               builder.append("\\\"");
               break;
            case '\\':
               builder.append("\\\\");
               break;
            case '\b':
               builder.append("\\b");
               break;
            case '\f':
               builder.append("\\f");
               break;
            case '\n':
               builder.append("\\n");
               break;
            case '\r':
               builder.append("\\r");
               break;
            case '\t':
               builder.append("\\t");
               break;
            default:
               if (character < 0x20) {
                  builder.append(String.format("\\u%04x", (int)character));
               } else {
                  builder.append(character);
               }
               break;
         }
      }
   }

   private static void printUsage() {
      System.err.println("usage: VanillaClientLauncher --mclone-game-dir <path> --mclone-assets-dir <path> --mclone-asset-index <id> [options]");
      System.err.println();
      System.err.println("options:");
      System.err.println("  --mclone-print-args");
      System.err.println("  --mclone-launch");
      System.err.println("  --mclone-username <name>");
      System.err.println("  --mclone-uuid <uuid>");
      System.err.println("  --mclone-access-token <token>");
      System.err.println("  --mclone-width <pixels>");
      System.err.println("  --mclone-height <pixels>");
      System.err.println("  --mclone-screenshot <png>");
      System.err.println("  --mclone-seed <seed>");
      System.err.println("  --mclone-world-name <name>");
      System.err.println("  --mclone-camera <x,y,z,yaw,pitch>");
      System.err.println("  --mclone-day-time <ticks>");
      System.err.println("  --mclone-settle-frames <frames>");
      System.err.println("  --mclone-timeout-seconds <seconds>");
      System.err.println("  --mclone-render-distance <chunks>");
      System.err.println("  --mclone-show-gui");
      System.err.println("  --mclone-no-structures");
      System.err.println("  --mclone-disable-multiplayer");
      System.err.println("  --mclone-disable-chat");
   }

   private static final class Options {
      boolean launch;
      boolean printArgs;
      String gameDir;
      String assetsDir;
      String assetIndex;
      String username;
      String uuid;
      String accessToken;
      String version;
      String userType;
      String versionType;
      int width = DEFAULT_WIDTH;
      int height = DEFAULT_HEIGHT;
      String screenshotPath;
      long seed = 12345L;
      String worldName = "McloneOracleWorld";
      double cameraX = 0.0;
      double cameraY = 96.0;
      double cameraZ = 0.0;
      float yaw = 180.0F;
      float pitch = 20.0F;
      long dayTime = 6000L;
      int settleFrames = 80;
      int timeoutSeconds = 180;
      int renderDistance = 12;
      boolean generateStructures = true;
      boolean hideGui = true;
      boolean disableMultiplayer;
      boolean disableChat;

      void setCamera(String value) {
         String[] parts = value.split(",");
         if (parts.length != 5) {
            throw new IllegalArgumentException("camera must use x,y,z,yaw,pitch");
         }
         cameraX = parseDouble(parts[0], "camera x");
         cameraY = parseDouble(parts[1], "camera y");
         cameraZ = parseDouble(parts[2], "camera z");
         yaw = (float)parseDouble(parts[3], "camera yaw");
         pitch = (float)parseDouble(parts[4], "camera pitch");
      }

      VanillaClientScreenshotHarness.Config screenshotConfig() {
         VanillaClientScreenshotHarness.Config config = new VanillaClientScreenshotHarness.Config();
         config.gameDir = Paths.get(gameDir);
         config.output = Paths.get(screenshotPath);
         config.worldName = worldName;
         config.seed = seed;
         config.cameraX = cameraX;
         config.cameraY = cameraY;
         config.cameraZ = cameraZ;
         config.yaw = yaw;
         config.pitch = pitch;
         config.dayTime = dayTime;
         config.settleFrames = settleFrames;
         config.timeoutSeconds = timeoutSeconds;
         config.renderDistance = renderDistance;
         config.generateStructures = generateStructures;
         config.hideGui = hideGui;
         return config;
      }
   }

   private static int parseRenderDistance(String value) {
      int parsed = parsePositiveInt(value, "render distance");
      if (parsed < 2 || parsed > 16) {
         throw new IllegalArgumentException("render distance must be between 2 and 16");
      }
      return parsed;
   }
}
