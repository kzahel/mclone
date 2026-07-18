import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.nio.ByteBuffer;
import java.security.MessageDigest;
import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.world.World;
import net.minecraft.world.biome.Biome;
import net.minecraft.world.biome.source.BiomeSource;
import net.minecraft.world.dimension.OverworldDimension;
import net.minecraft.world.gen.chunk.OverworldChunkGenerator;
import net.minecraft.world.storage.EmptyWorldStorage;

/**
 * Deterministic staged probe for the Feather-mapped Beta 1.7.3 Overworld.
 *
 * <p>The probe stops before delayed population. It fingerprints the climate
 * inputs and semantic biome grid alongside density terrain, surfaces, or
 * caves in Beta's native X/Z/Y byte order.
 */
public final class BetaTerrainProbe {
    private BetaTerrainProbe() {
    }

    public static void main(String[] args) throws Exception {
        long seed = 12345L;
        int chunkX = 0;
        int chunkZ = 0;
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

        PrintStream stdout = System.out;
        World world;
        Biome[] effectiveBiomes;
        try {
            // Beta initializes achievements and recipes while the block and
            // biome singletons load. Keep those historical startup messages
            // out of the probe's machine-readable JSON.
            System.setOut(new PrintStream(new ByteArrayOutputStream()));
            world = new World(
                new EmptyWorldStorage(),
                "mclone-beta-probe",
                new OverworldDimension(),
                seed
            );
            effectiveBiomes = effectiveBiomes();
        } finally {
            System.setOut(stdout);
        }
        BiomeSource biomeSource = world.getBiomeSource();
        Biome[] biomes = biomeSource.getBiomes(null, chunkX * 16, chunkZ * 16, 16, 16);
        double[] temperatures = biomeSource.temperatures.clone();
        double[] downfalls = biomeSource.downfalls.clone();
        byte[] biomeIds = semanticBiomeIds(biomes, effectiveBiomes);

        OverworldChunkGenerator generator = new OverworldChunkGenerator(world, seed);
        byte[] blocks;
        if (stage.equals("caves")) {
            blocks = generator.getChunk(chunkX, chunkZ).blocks;
        } else {
            blocks = new byte[32768];
            generator.buildTerrain(chunkX, chunkZ, blocks, biomes, temperatures);
            if (stage.equals("surface")) {
                java.lang.reflect.Field field = OverworldChunkGenerator.class.getDeclaredField("random");
                field.setAccessible(true);
                ((java.util.Random) field.get(generator)).setSeed(
                    chunkX * 341873128712L + chunkZ * 132897987541L
                );
                generator.buildSurfaces(chunkX, chunkZ, blocks, biomes);
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

        Map<String, Integer> biomeCounts = new LinkedHashMap<>();
        for (Biome biome : effectiveBiomes) {
            biomeCounts.put(slug(biome.name), 0);
        }
        for (Biome biome : biomes) {
            String name = slug(biome.name);
            biomeCounts.put(name, biomeCounts.get(name) + 1);
        }

        System.out.println("{");
        System.out.println("  \"version\": \"b1.7.3\",");
        System.out.println("  \"stage\": \"" + stage + "\",");
        System.out.println("  \"seed\": " + seed + ",");
        System.out.println("  \"chunk_x\": " + chunkX + ",");
        System.out.println("  \"chunk_z\": " + chunkZ + ",");
        System.out.println("  \"sha256\": \"" + sha256(blocks) + "\",");
        System.out.println("  \"biome_sha256\": \"" + sha256(biomeIds) + "\",");
        System.out.println("  \"temperature_sha256_be\": \"" + sha256(doublesToBytes(temperatures)) + "\",");
        System.out.println("  \"downfall_sha256_be\": \"" + sha256(doublesToBytes(downfalls)) + "\",");
        System.out.printf("  \"temperature_min\": %.12f,%n", minimum(temperatures));
        System.out.printf("  \"temperature_max\": %.12f,%n", maximum(temperatures));
        System.out.printf("  \"temperature_mean\": %.12f,%n", mean(temperatures));
        System.out.printf("  \"downfall_min\": %.12f,%n", minimum(downfalls));
        System.out.printf("  \"downfall_max\": %.12f,%n", maximum(downfalls));
        System.out.printf("  \"downfall_mean\": %.12f,%n", mean(downfalls));
        System.out.println("  \"height_min\": " + minHeight + ",");
        System.out.println("  \"height_max\": " + maxHeight + ",");
        System.out.printf("  \"height_mean\": %.6f,%n", heightSum / 256.0);
        System.out.println("  \"biomes\": {");
        int biomeIndex = 0;
        for (Map.Entry<String, Integer> entry : biomeCounts.entrySet()) {
            String suffix = ++biomeIndex == biomeCounts.size() ? "" : ",";
            System.out.println("    \"" + entry.getKey() + "\": " + entry.getValue() + suffix);
        }
        System.out.println("  },");
        System.out.println("  \"blocks\": {");
        System.out.println("    \"air_0\": " + blockCounts[0] + ",");
        System.out.println("    \"stone_1\": " + blockCounts[1] + ",");
        System.out.println("    \"grass_2\": " + blockCounts[2] + ",");
        System.out.println("    \"dirt_3\": " + blockCounts[3] + ",");
        System.out.println("    \"bedrock_7\": " + blockCounts[7] + ",");
        System.out.println("    \"water_9\": " + blockCounts[9] + ",");
        System.out.println("    \"lava_11\": " + blockCounts[11] + ",");
        System.out.println("    \"sand_12\": " + blockCounts[12] + ",");
        System.out.println("    \"gravel_13\": " + blockCounts[13] + ",");
        System.out.println("    \"sandstone_24\": " + blockCounts[24] + ",");
        System.out.println("    \"ice_79\": " + blockCounts[79]);
        System.out.println("  }");
        System.out.println("}");
    }

    private static Biome[] effectiveBiomes() {
        return new Biome[]{
            Biome.RAINFOREST,
            Biome.SWAMPLAND,
            Biome.SEASONAL_FOREST,
            Biome.FOREST,
            Biome.SAVANNA,
            Biome.SHRUBLAND,
            Biome.TAIGA,
            Biome.DESERT,
            Biome.PLAINS,
            Biome.TUNDRA,
        };
    }

    private static byte[] semanticBiomeIds(Biome[] biomes, Biome[] effectiveBiomes) {
        byte[] ids = new byte[biomes.length];
        for (int i = 0; i < biomes.length; i++) {
            ids[i] = (byte)biomeIndex(biomes[i], effectiveBiomes);
        }
        return ids;
    }

    private static int biomeIndex(Biome biome, Biome[] effectiveBiomes) {
        for (int i = 0; i < effectiveBiomes.length; i++) {
            if (biome == effectiveBiomes[i]) {
                return i;
            }
        }
        throw new IllegalArgumentException("unexpected Overworld biome: " + biome.name);
    }

    private static String slug(String value) {
        return value.toLowerCase().replace(' ', '_');
    }

    private static byte[] doublesToBytes(double[] values) {
        ByteBuffer buffer = ByteBuffer.allocate(values.length * Double.BYTES);
        for (double value : values) {
            buffer.putDouble(value);
        }
        return buffer.array();
    }

    private static double minimum(double[] values) {
        double result = Double.POSITIVE_INFINITY;
        for (double value : values) {
            result = Math.min(result, value);
        }
        return result;
    }

    private static double maximum(double[] values) {
        double result = Double.NEGATIVE_INFINITY;
        for (double value : values) {
            result = Math.max(result, value);
        }
        return result;
    }

    private static double mean(double[] values) {
        double result = 0.0;
        for (double value : values) {
            result += value;
        }
        return result / values.length;
    }

    private static String requireValue(String[] args, int index, String option) {
        if (index >= args.length) {
            throw new IllegalArgumentException(option + " requires a value");
        }
        return args[index];
    }

    private static void printUsage() {
        System.out.println(
            "Usage: BetaTerrainProbe [--seed N] [--chunk-x N] [--chunk-z N] "
                + "[--stage terrain|surface|caves]"
        );
    }

    private static String sha256(byte[] data) throws Exception {
        byte[] digest = MessageDigest.getInstance("SHA-256").digest(data);
        StringBuilder result = new StringBuilder(digest.length * 2);
        for (byte value : digest) {
            result.append(String.format("%02x", value & 0xFF));
        }
        return result.toString();
    }
}
