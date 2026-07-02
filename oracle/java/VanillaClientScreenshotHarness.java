import com.mojang.blaze3d.platform.NativeImage;
import java.io.IOException;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.Properties;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import java.util.function.Supplier;
import java.util.stream.Stream;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Screenshot;
import net.minecraft.core.BlockPos;
import net.minecraft.core.RegistryAccess;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.Difficulty;
import net.minecraft.world.level.DataPackConfig;
import net.minecraft.world.level.GameRules;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.LevelSettings;
import net.minecraft.world.level.levelgen.WorldGenSettings;

public final class VanillaClientScreenshotHarness {
   private static final long POLL_INTERVAL_MILLIS = 50L;
   private static final long GAME_THREAD_TIMEOUT_SECONDS = 60L;
   private static final Field OVERLAY_FIELD = findOverlayField();

   private VanillaClientScreenshotHarness() {
   }

   public static void start(Config config) {
      Thread thread = new Thread(() -> run(config), "mclone-vanilla-screenshot");
      thread.setDaemon(true);
      thread.start();
   }

   private static void run(Config config) {
      try {
         deleteRecursively(config.gameDir.resolve("saves").resolve(config.worldName));
         Minecraft minecraft = waitForMinecraft(config);
         waitUntil(config, () -> onGameThread(minecraft, () -> minecraft.level == null && overlay(minecraft) == null), "client startup");
         createWorld(config, minecraft);
         waitUntil(
            config,
            () -> onGameThread(
               minecraft,
               () -> minecraft.level != null
                  && minecraft.player != null
                  && minecraft.getSingleplayerServer() != null
                  && minecraft.getSingleplayerServer().isReady()
            ),
            "singleplayer world"
         );
         configureWorldAndCamera(config, minecraft);
         BlockPos targetPos = new BlockPos(config.cameraX, config.cameraY, config.cameraZ);
         waitUntil(config, () -> onGameThread(minecraft, () -> minecraft.level != null && minecraft.level.hasChunkAt(targetPos)), "target chunk");
         sleep(Math.max(0, config.settleFrames) * POLL_INTERVAL_MILLIS);
         capture(config, minecraft);
      } catch (Throwable error) {
         error.printStackTrace(System.err);
         tryStopMinecraft();
         Runtime.getRuntime().halt(1);
      }
   }

   private static Minecraft waitForMinecraft(Config config) throws InterruptedException {
      long deadline = deadlineNanos(config);
      while (System.nanoTime() < deadline) {
         Minecraft minecraft = Minecraft.getInstance();
         if (minecraft != null) {
            return minecraft;
         }
         Thread.sleep(POLL_INTERVAL_MILLIS);
      }
      throw new IllegalStateException("timed out waiting for Minecraft instance");
   }

   private static void createWorld(Config config, Minecraft minecraft) throws Exception {
      onGameThread(
         minecraft,
         () -> {
            minecraft.options.renderDistance = config.renderDistance;
            RegistryAccess.RegistryHolder registry = RegistryAccess.builtin();
            Properties properties = new Properties();
            properties.setProperty("level-seed", Long.toString(config.seed));
            properties.setProperty("generate-structures", Boolean.toString(config.generateStructures));
            properties.setProperty("level-type", "default");
            properties.setProperty("generator-settings", "");
            WorldGenSettings worldGenSettings = WorldGenSettings.create(registry, properties);
            LevelSettings levelSettings = new LevelSettings(
               config.worldName,
               GameType.CREATIVE,
               false,
               Difficulty.PEACEFUL,
               true,
               new GameRules(),
               DataPackConfig.DEFAULT
            );
            minecraft.createLevel(config.worldName, levelSettings, registry, worldGenSettings);
            return null;
         }
      );
   }

   private static void configureWorldAndCamera(Config config, Minecraft minecraft) throws Exception {
      onGameThread(
         minecraft,
         () -> {
            MinecraftServer server = minecraft.getSingleplayerServer();
            if (server != null) {
               ServerLevel overworld = server.overworld();
               overworld.setDayTime(config.dayTime);
               overworld.setWeatherParameters(1000000, 0, false, false);
            }
            minecraft.options.hideGui = config.hideGui;
            minecraft.options.renderDebug = false;
            minecraft.options.renderDebugCharts = false;
            minecraft.options.renderFpsChart = false;
            minecraft.options.renderDistance = config.renderDistance;
            minecraft.options.framerateLimit = 260;
            minecraft.getWindow().setFramerateLimit(minecraft.options.framerateLimit);
            minecraft.setScreen(null);
            minecraft.player.moveTo(config.cameraX, config.cameraY, config.cameraZ, config.yaw, config.pitch);
            minecraft.player.yHeadRot = config.yaw;
            minecraft.player.yBodyRot = config.yaw;
            minecraft.player.yHeadRotO = config.yaw;
            minecraft.player.yBodyRotO = config.yaw;
            minecraft.player.setOldPosAndRot();
            minecraft.player.getAbilities().mayfly = true;
            minecraft.player.getAbilities().flying = true;
            minecraft.player.getAbilities().invulnerable = true;
            minecraft.setCameraEntity(minecraft.player);
            return null;
         }
      );
   }

   private static void capture(Config config, Minecraft minecraft) throws Exception {
      onGameThread(
         minecraft,
         () -> {
            try {
               Path parent = config.output.getParent();
               if (parent != null) {
                  Files.createDirectories(parent);
               }
               NativeImage image = Screenshot.takeScreenshot(minecraft.getMainRenderTarget());
               try {
                  image.writeToFile(config.output);
               } finally {
                  image.close();
               }
               System.out.println("mclone vanilla screenshot saved: " + config.output.toAbsolutePath());
            } catch (IOException error) {
               throw new RuntimeException(error);
            } finally {
               minecraft.stop();
            }
            return null;
         }
      );
   }

   private static <T> T onGameThread(Minecraft minecraft, Supplier<T> supplier) throws Exception {
      CompletableFuture<T> future = minecraft.submit(supplier);
      return future.get(GAME_THREAD_TIMEOUT_SECONDS, TimeUnit.SECONDS);
   }

   private static void waitUntil(Config config, CheckedBooleanSupplier condition, String description) throws Exception {
      long deadline = deadlineNanos(config);
      while (System.nanoTime() < deadline) {
         if (condition.getAsBoolean()) {
            return;
         }
         Thread.sleep(POLL_INTERVAL_MILLIS);
      }
      throw new IllegalStateException("timed out waiting for " + description);
   }

   private static long deadlineNanos(Config config) {
      return System.nanoTime() + TimeUnit.SECONDS.toNanos(Math.max(1, config.timeoutSeconds));
   }

   private static Object overlay(Minecraft minecraft) {
      if (OVERLAY_FIELD == null) {
         return null;
      }
      try {
         return OVERLAY_FIELD.get(minecraft);
      } catch (IllegalAccessException error) {
         throw new RuntimeException(error);
      }
   }

   private static Field findOverlayField() {
      try {
         Field field = Minecraft.class.getDeclaredField("overlay");
         field.setAccessible(true);
         return field;
      } catch (ReflectiveOperationException error) {
         return null;
      }
   }

   private static void deleteRecursively(Path path) throws IOException {
      if (!Files.exists(path)) {
         return;
      }
      try (Stream<Path> stream = Files.walk(path)) {
         for (Path entry : stream.sorted(Comparator.reverseOrder()).toList()) {
            Files.deleteIfExists(entry);
         }
      }
   }

   private static void tryStopMinecraft() {
      try {
         Minecraft minecraft = Minecraft.getInstance();
         if (minecraft != null) {
            minecraft.submit(
               () -> {
                  minecraft.stop();
                  return null;
               }
            );
         }
      } catch (Throwable ignored) {
      }
   }

   private static void sleep(long millis) throws InterruptedException {
      if (millis > 0) {
         Thread.sleep(millis);
      }
   }

   @FunctionalInterface
   private interface CheckedBooleanSupplier {
      boolean getAsBoolean() throws Exception;
   }

   public static final class Config {
      Path gameDir;
      Path output;
      String worldName = "McloneOracleWorld";
      long seed = 12345L;
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
   }
}
