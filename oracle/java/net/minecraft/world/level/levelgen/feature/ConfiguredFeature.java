package net.minecraft.world.level.levelgen.feature;

import com.mojang.serialization.Codec;
import com.mojang.serialization.JsonOps;
import java.util.List;
import java.util.Objects;
import java.util.Random;
import java.util.function.Supplier;
import java.util.stream.Stream;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Registry;
import net.minecraft.data.BuiltinRegistries;
import net.minecraft.resources.RegistryFileCodec;
import net.minecraft.world.level.WorldGenLevel;
import net.minecraft.world.level.chunk.ChunkGenerator;
import net.minecraft.world.level.chunk.McloneSchedulerTraceRecorder;
import net.minecraft.world.level.levelgen.Decoratable;
import net.minecraft.world.level.levelgen.feature.configurations.DecoratedFeatureConfiguration;
import net.minecraft.world.level.levelgen.feature.configurations.FeatureConfiguration;
import net.minecraft.world.level.levelgen.placement.ConfiguredDecorator;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

// Oracle shadow: instrument final tree placements while preserving vanilla 1.17.1 ConfiguredFeature logic.
public class ConfiguredFeature<FC extends FeatureConfiguration, F extends Feature<FC>> implements Decoratable<ConfiguredFeature<?, ?>> {
   public static final Codec<ConfiguredFeature<?, ?>> DIRECT_CODEC = Registry.FEATURE.dispatch(var0 -> var0.feature, Feature::configuredCodec);
   public static final Codec<Supplier<ConfiguredFeature<?, ?>>> CODEC = RegistryFileCodec.create(Registry.CONFIGURED_FEATURE_REGISTRY, DIRECT_CODEC);
   public static final Codec<List<Supplier<ConfiguredFeature<?, ?>>>> LIST_CODEC = RegistryFileCodec.homogeneousList(
      Registry.CONFIGURED_FEATURE_REGISTRY, DIRECT_CODEC
   );
   public static final Logger LOGGER = LogManager.getLogger();
   public final F feature;
   public final FC config;

   public ConfiguredFeature(F var1, FC var2) {
      this.feature = (F)var1;
      this.config = (FC)var2;
   }

   public F feature() {
      return this.feature;
   }

   public FC config() {
      return this.config;
   }

   public ConfiguredFeature<?, ?> decorated(ConfiguredDecorator<?> var1) {
      return Feature.DECORATED.configured(new DecoratedFeatureConfiguration(() -> this, var1));
   }

   public WeightedConfiguredFeature weighted(float var1) {
      return new WeightedConfiguredFeature(this, var1);
   }

   public boolean place(WorldGenLevel var1, ChunkGenerator var2, Random var3, BlockPos var4) {
      String featureType = Registry.FEATURE.getKey(this.feature()).toString();
      boolean traceTree = "minecraft:tree".equals(featureType)
         && McloneSchedulerTraceRecorder.beginTreeCandidate(this.toString(), featureType, var3, var4);
      FeaturePlaceContext<FC> context = new FeaturePlaceContext<>(var1, var2, var3, var4, this.config);
      if (!traceTree) {
         return this.feature.place(context);
      }

      try {
         boolean result = this.feature.place(context);
         McloneSchedulerTraceRecorder.finishTreeCandidate(result, var3, null);
         return result;
      } catch (RuntimeException | Error error) {
         McloneSchedulerTraceRecorder.finishTreeCandidate(false, var3, error);
         throw error;
      }
   }

   public Stream<ConfiguredFeature<?, ?>> getFeatures() {
      return Stream.concat(Stream.of(this), this.config.getFeatures());
   }

   @Override
   public String toString() {
      return BuiltinRegistries.CONFIGURED_FEATURE
         .getResourceKey(this)
         .map(Objects::toString)
         .orElseGet(() -> DIRECT_CODEC.encodeStart(JsonOps.INSTANCE, this).toString());
   }
}
