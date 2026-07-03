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
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.SectionPos;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.Difficulty;
import net.minecraft.world.level.DataPackConfig;
import net.minecraft.world.level.GameRules;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.LevelSettings;
import net.minecraft.world.level.chunk.ChunkStatus;
import net.minecraft.world.level.levelgen.WorldGenSettings;

public final class VanillaClientScreenshotHarness {
   private static final long POLL_INTERVAL_MILLIS = 50L;
   private static final long GAME_THREAD_TIMEOUT_SECONDS = 60L;
   private static final Field OVERLAY_FIELD = findOverlayField();
   private static final Field PAUSE_FIELD = findPauseField();

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
         configureEarlyCaptureOptions(minecraft);
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
         waitUntil(config, () -> onGameThread(minecraft, () -> hasLoadedClientChunks(minecraft, config)), "camera chunk radius");
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
            minecraft.options.hideGui = config.hideGui;
            minecraft.options.pauseOnLostFocus = false;
            minecraft.options.renderDebug = false;
            minecraft.options.renderDebugCharts = false;
            minecraft.options.renderFpsChart = false;
            minecraft.options.renderDistance = config.renderDistance;
            minecraft.options.framerateLimit = 260;
            minecraft.getWindow().setFramerateLimit(minecraft.options.framerateLimit);
            minecraft.setScreen(null);
            setPaused(minecraft, false);
            return null;
         }
      );

      MinecraftServer server = onGameThread(minecraft, minecraft::getSingleplayerServer);
      if (server == null) {
         throw new IllegalStateException("singleplayer server is not available");
      }

      onServerThread(
         server,
         () -> {
            ServerLevel overworld = server.overworld();
            overworld.setDayTime(config.dayTime);
            overworld.setWeatherParameters(1000000, 0, false, false);
            if (server.getPlayerList().getPlayers().isEmpty()) {
               throw new IllegalStateException("singleplayer server has no player");
            }
            ServerPlayer player = server.getPlayerList().getPlayers().get(0);
            player.connection.teleport(config.cameraX, config.cameraY, config.cameraZ, config.yaw, config.pitch);
            player.yHeadRot = config.yaw;
            player.yBodyRot = config.yaw;
            player.yHeadRotO = config.yaw;
            player.yBodyRotO = config.yaw;
            player.setOldPosAndRot();
            player.connection.resetPosition();
            player.getAbilities().mayfly = true;
            player.getAbilities().flying = true;
            player.getAbilities().invulnerable = true;
            player.onUpdateAbilities();
            player.getLevel().getChunkSource().move(player);
            return null;
         }
      );

      onGameThread(
         minecraft,
         () -> {
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
            minecraft.setScreen(null);
            setPaused(minecraft, false);
            System.out.println(
               "mclone vanilla camera configured: seed="
                  + config.seed
                  + " player=("
                  + config.cameraX
                  + ", "
                  + config.cameraY
                  + ", "
                  + config.cameraZ
                  + ") chunk=("
                  + SectionPos.posToSectionCoord(config.cameraX)
                  + ", "
                  + SectionPos.posToSectionCoord(config.cameraZ)
                  + ") waitRadius="
                  + chunkWaitRadius(config)
            );
            return null;
         }
      );
   }

   private static void configureEarlyCaptureOptions(Minecraft minecraft) throws Exception {
      onGameThread(
         minecraft,
         () -> {
            minecraft.options.pauseOnLostFocus = false;
            setPaused(minecraft, false);
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

   private static <T> T onServerThread(MinecraftServer server, Supplier<T> supplier) throws Exception {
      CompletableFuture<T> future = server.submit(supplier);
      return future.get(GAME_THREAD_TIMEOUT_SECONDS, TimeUnit.SECONDS);
   }

   private static boolean hasLoadedClientChunks(Minecraft minecraft, Config config) {
      if (minecraft.level == null) {
         return false;
      }
      int centerX = SectionPos.posToSectionCoord(config.cameraX);
      int centerZ = SectionPos.posToSectionCoord(config.cameraZ);
      int radius = chunkWaitRadius(config);
      for (int z = centerZ - radius; z <= centerZ + radius; z++) {
         for (int x = centerX - radius; x <= centerX + radius; x++) {
            if (minecraft.level.getChunkSource().getChunk(x, z, ChunkStatus.FULL, false) == null) {
               return false;
            }
         }
      }
      System.out.println("mclone vanilla chunks ready: center=(" + centerX + ", " + centerZ + ") radius=" + radius);
      return true;
   }

   private static int chunkWaitRadius(Config config) {
      return Math.min(1, Math.max(0, config.renderDistance));
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

   private static void setPaused(Minecraft minecraft, boolean paused) {
      if (PAUSE_FIELD == null) {
         return;
      }
      try {
         PAUSE_FIELD.setBoolean(minecraft, paused);
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

   private static Field findPauseField() {
      try {
         Field field = Minecraft.class.getDeclaredField("pause");
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
