import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Comparator;
import net.minecraft.world.World;
import net.minecraft.world.chunk.WorldChunk;
import net.minecraft.world.gen.chunk.OverworldChunkGenerator;

/**
 * Small headless probe for the Feather-mapped Alpha terrain generator.
 *
 * <p>The probe can stop after density terrain, surfaces, or caves. Population
 * remains a separate world-mutating pass and is intentionally excluded.
 */
public final class AlphaTerrainProbe {
    private AlphaTerrainProbe() {
    }

    public static void main(String[] args) throws Exception {
        long seed = 12345L;
        int chunkX = 0;
        int chunkZ = 0;
        boolean snow = false;
        String stage = "caves";

        for (int i = 0; i < args.length; i++) {
            switch (args[i]) {
                case "--seed":
                    seed = Long.parseLong(requireValue(args, ++i, "--seed"));
                    break;
                case "--chunk-x":
                    chunkX = Integer.parseInt(requireValue(args, ++i, "--chunk-x"));
                    break;
                case "--chunk-z":
                    chunkZ = Integer.parseInt(requireValue(args, ++i, "--chunk-z"));
                    break;
                case "--snow":
                    snow = Boolean.parseBoolean(requireValue(args, ++i, "--snow"));
                    break;
                case "--stage":
                    stage = requireValue(args, ++i, "--stage");
                    if (!stage.equals("terrain") && !stage.equals("surface") && !stage.equals("caves")) {
                        throw new IllegalArgumentException("--stage must be terrain, surface, or caves");
                    }
                    break;
                case "--help":
                    printUsage();
                    return;
                default:
                    throw new IllegalArgumentException("unknown argument: " + args[i]);
            }
        }

        Path scratch = Files.createTempDirectory("mclone-alpha-terrain-probe-");
        try {
            World world = new World(scratch.toFile(), "World1", seed);
            // Real Alpha chooses whole-world winter mode from an unseeded
            // Random. Override it so probe fingerprints are reproducible.
            world.snowCovered = snow;
            OverworldChunkGenerator generator = new OverworldChunkGenerator(world, seed);
            byte[] blocks;
            if (stage.equals("caves")) {
                blocks = generator.getChunk(chunkX, chunkZ).blocks;
            } else {
                blocks = new byte[32768];
                generator.buildTerrain(chunkX, chunkZ, blocks);
                if (stage.equals("surface")) {
                    // getChunk performs this reseed before both stages.
                    // buildTerrain itself consumes no chunk-local randomness.
                    java.lang.reflect.Field field = OverworldChunkGenerator.class.getDeclaredField("random");
                    field.setAccessible(true);
                    ((java.util.Random) field.get(generator)).setSeed(
                        chunkX * 341873128712L + chunkZ * 132897987541L
                    );
                    generator.buildSurfaces(chunkX, chunkZ, blocks);
                }
            }

            int[] blockCounts = new int[256];
            for (byte block : blocks) {
                blockCounts[block & 0xFF]++;
            }

            int minHeight = 255;
            int maxHeight = 0;
            long heightSum = 0;
            for (int x = 0; x < 16; x++) {
                for (int z = 0; z < 16; z++) {
                    int value = 0;
                    for (int y = 127; y >= 0; y--) {
                        if (blocks[(x * 16 + z) * 128 + y] != 0) {
                            value = y + 1;
                            break;
                        }
                    }
                minHeight = Math.min(minHeight, value);
                maxHeight = Math.max(maxHeight, value);
                heightSum += value;
                }
            }

            System.out.println("{");
            System.out.println("  \"stage\": \"" + stage + "\",");
            System.out.println("  \"seed\": " + seed + ",");
            System.out.println("  \"chunk_x\": " + chunkX + ",");
            System.out.println("  \"chunk_z\": " + chunkZ + ",");
            System.out.println("  \"snow\": " + snow + ",");
            System.out.println("  \"sha256\": \"" + sha256(blocks) + "\",");
            System.out.println("  \"height_min\": " + minHeight + ",");
            System.out.println("  \"height_max\": " + maxHeight + ",");
            System.out.printf("  \"height_mean\": %.6f,%n", heightSum / 256.0);
            System.out.println("  \"blocks\": {");
            System.out.println("    \"air_0\": " + blockCounts[0] + ",");
            System.out.println("    \"stone_1\": " + blockCounts[1] + ",");
            System.out.println("    \"grass_2\": " + blockCounts[2] + ",");
            System.out.println("    \"dirt_3\": " + blockCounts[3] + ",");
            System.out.println("    \"bedrock_7\": " + blockCounts[7] + ",");
            System.out.println("    \"flowing_water_8\": " + blockCounts[8] + ",");
            System.out.println("    \"water_9\": " + blockCounts[9] + ",");
            System.out.println("    \"flowing_lava_10\": " + blockCounts[10] + ",");
            System.out.println("    \"lava_11\": " + blockCounts[11] + ",");
            System.out.println("    \"sand_12\": " + blockCounts[12] + ",");
            System.out.println("    \"gravel_13\": " + blockCounts[13] + ",");
            System.out.println("    \"ice_79\": " + blockCounts[79]);
            System.out.println("  }");
            System.out.println("}");
        } finally {
            deleteRecursively(scratch);
        }
    }

    private static String requireValue(String[] args, int index, String option) {
        if (index >= args.length) {
            throw new IllegalArgumentException(option + " requires a value");
        }
        return args[index];
    }

    private static void printUsage() {
        System.out.println("Usage: AlphaTerrainProbe [--seed N] [--chunk-x N] [--chunk-z N] [--snow true|false] [--stage terrain|surface|caves]");
    }

    private static String sha256(byte[] data) throws Exception {
        byte[] digest = MessageDigest.getInstance("SHA-256").digest(data);
        StringBuilder result = new StringBuilder(digest.length * 2);
        for (byte value : digest) {
            result.append(String.format("%02x", value & 0xFF));
        }
        return result.toString();
    }

    private static void deleteRecursively(Path root) throws Exception {
        if (!Files.exists(root)) {
            return;
        }
        try (var paths = Files.walk(root)) {
            paths.sorted(Comparator.reverseOrder()).map(Path::toFile).forEach(File::delete);
        }
    }
}
