package net.minecraft.world.level.levelgen.feature;

import com.google.common.collect.Iterables;
import com.google.common.collect.Lists;
import com.google.common.collect.Sets;
import com.mojang.serialization.Codec;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.OptionalInt;
import java.util.Random;
import java.util.Set;
import java.util.function.BiConsumer;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.Vec3i;
import net.minecraft.tags.BlockTags;
import net.minecraft.world.level.LevelAccessor;
import net.minecraft.world.level.LevelSimulatedReader;
import net.minecraft.world.level.LevelWriter;
import net.minecraft.world.level.WorldGenLevel;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.BlockStateProperties;
import net.minecraft.world.level.chunk.McloneSchedulerTraceRecorder;
import net.minecraft.world.level.levelgen.feature.configurations.TreeConfiguration;
import net.minecraft.world.level.levelgen.feature.foliageplacers.FoliagePlacer;
import net.minecraft.world.level.levelgen.structure.BoundingBox;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplate;
import net.minecraft.world.level.material.Material;
import net.minecraft.world.phys.shapes.BitSetDiscreteVoxelShape;
import net.minecraft.world.phys.shapes.DiscreteVoxelShape;

// Oracle shadow: record tree rejection details while preserving vanilla 1.17.1 TreeFeature logic.
public class TreeFeature extends Feature<TreeConfiguration> {
   private static final int BLOCK_UPDATE_FLAGS = 19;

   public TreeFeature(Codec<TreeConfiguration> var1) {
      super(var1);
   }

   public static boolean isFree(LevelSimulatedReader var0, BlockPos var1) {
      return validTreePos(var0, var1) || var0.isStateAtPosition(var1, var0x -> var0x.is(BlockTags.LOGS));
   }

   private static boolean isVine(LevelSimulatedReader var0, BlockPos var1) {
      return var0.isStateAtPosition(var1, var0x -> var0x.is(Blocks.VINE));
   }

   private static boolean isBlockWater(LevelSimulatedReader var0, BlockPos var1) {
      return var0.isStateAtPosition(var1, var0x -> var0x.is(Blocks.WATER));
   }

   public static boolean isAirOrLeaves(LevelSimulatedReader var0, BlockPos var1) {
      return var0.isStateAtPosition(var1, var0x -> var0x.isAir() || var0x.is(BlockTags.LEAVES));
   }

   private static boolean isReplaceablePlant(LevelSimulatedReader var0, BlockPos var1) {
      return var0.isStateAtPosition(var1, var0x -> {
         Material var1x = var0x.getMaterial();
         return var1x == Material.REPLACEABLE_PLANT;
      });
   }

   private static void setBlockKnownShape(LevelWriter var0, BlockPos var1, BlockState var2) {
      var0.setBlock(var1, var2, 19);
   }

   public static boolean validTreePos(LevelSimulatedReader var0, BlockPos var1) {
      return isAirOrLeaves(var0, var1) || isReplaceablePlant(var0, var1) || isBlockWater(var0, var1);
   }

   private boolean doPlace(
      WorldGenLevel var1, Random var2, BlockPos var3, BiConsumer<BlockPos, BlockState> var4, BiConsumer<BlockPos, BlockState> var5, TreeConfiguration var6
   ) {
      int var7 = var6.trunkPlacer.getTreeHeight(var2);
      int var8 = var6.foliagePlacer.foliageHeight(var2, var7, var6);
      int var9 = var7 - var8;
      int var10 = var6.foliagePlacer.foliageRadius(var2, var9);
      McloneSchedulerTraceRecorder.recordTreeCandidateConfiguration(
         var6.trunkPlacer.getClass().getSimpleName(),
         var6.foliagePlacer.getClass().getSimpleName(),
         var6.minimumSize.getClass().getSimpleName()
      );
      McloneSchedulerTraceRecorder.recordTreeCandidateParameters(var7, var8, var9, var10);
      if (var3.getY() < var1.getMinBuildHeight() + 1 || var3.getY() + var7 + 1 > var1.getMaxBuildHeight()) {
         McloneSchedulerTraceRecorder.recordTreeCandidateRejected("bounds");
         return false;
      } else {
         BlockState var11 = var6.saplingProvider.getState(var2, var3);
         boolean var12 = var11.canSurvive(var1, var3);
         McloneSchedulerTraceRecorder.recordTreeCandidateSaplingState(var11, var12);
         if (!var12) {
            McloneSchedulerTraceRecorder.recordTreeCandidateRejected("sapling_cannot_survive");
            return false;
         } else {
            OptionalInt var13 = var6.minimumSize.minClippedHeight();
            int var14 = this.getMaxFreeTreeHeight(var1, var7, var3, var6);
            McloneSchedulerTraceRecorder.recordTreeCandidateHeightCheck(var14, var13);
            if (var14 >= var7 || var13.isPresent() && var14 >= var13.getAsInt()) {
               McloneSchedulerTraceRecorder.recordTreeCandidateAccepted(var14);
               List<FoliagePlacer.FoliageAttachment> var15 = var6.trunkPlacer.placeTrunk(var1, var4, var2, var14, var3, var6);
               var15.forEach(var7x -> var6.foliagePlacer.createFoliage(var1, var5, var2, var6, var14, var7x, var8, var10));
               return true;
            } else {
               McloneSchedulerTraceRecorder.recordTreeCandidateRejected("max_free_tree_height");
               return false;
            }
         }
      }
   }

   private int getMaxFreeTreeHeight(LevelSimulatedReader var1, int var2, BlockPos var3, TreeConfiguration var4) {
      BlockPos.MutableBlockPos var5 = new BlockPos.MutableBlockPos();

      for (int var6 = 0; var6 <= var2 + 1; var6++) {
         int var7 = var4.minimumSize.getSizeAtHeight(var2, var6);

         for (int var8 = -var7; var8 <= var7; var8++) {
            for (int var9 = -var7; var9 <= var7; var9++) {
               var5.setWithOffset(var3, var8, var6, var9);
               if (!isFree(var1, var5)) {
                  McloneSchedulerTraceRecorder.recordTreeCandidateBlockedPosition(
                     var5.immutable(),
                     blockStateForTraceIfActive(var1, var5),
                     false,
                     var6,
                     var7
                  );
                  return var6 - 2;
               }
               if (!var4.ignoreVines && isVine(var1, var5)) {
                  McloneSchedulerTraceRecorder.recordTreeCandidateBlockedPosition(
                     var5.immutable(),
                     blockStateForTraceIfActive(var1, var5),
                     true,
                     var6,
                     var7
                  );
                  return var6 - 2;
               }
            }
         }
      }

      return var2;
   }

   private static BlockState blockStateForTraceIfActive(LevelSimulatedReader var0, BlockPos var1) {
      return McloneSchedulerTraceRecorder.hasActiveTreeCandidate() && var0 instanceof WorldGenLevel ? ((WorldGenLevel)var0).getBlockState(var1) : null;
   }

   @Override
   protected void setBlock(LevelWriter var1, BlockPos var2, BlockState var3) {
      setBlockKnownShape(var1, var2, var3);
   }

   @Override
   public final boolean place(FeaturePlaceContext<TreeConfiguration> var1) {
      WorldGenLevel var2 = var1.level();
      Random var3 = var1.random();
      BlockPos var4 = var1.origin();
      TreeConfiguration var5 = (TreeConfiguration)var1.config();
      HashSet<BlockPos> var6 = Sets.newHashSet();
      HashSet<BlockPos> var7 = Sets.newHashSet();
      HashSet<BlockPos> var8 = Sets.newHashSet();
      BiConsumer<BlockPos, BlockState> var9 = (var2x, var3x) -> {
         var6.add(var2x.immutable());
         var2.setBlock(var2x, var3x, 19);
      };
      BiConsumer<BlockPos, BlockState> var10 = (var2x, var3x) -> {
         var7.add(var2x.immutable());
         var2.setBlock(var2x, var3x, 19);
      };
      BiConsumer<BlockPos, BlockState> var11 = (var2x, var3x) -> {
         var8.add(var2x.immutable());
         var2.setBlock(var2x, var3x, 19);
      };
      boolean var12 = this.doPlace(var2, var3, var4, var9, var10, var5);
      if (var12 && (!var6.isEmpty() || !var7.isEmpty())) {
         if (!var5.decorators.isEmpty()) {
            ArrayList<BlockPos> var13 = Lists.newArrayList(var6);
            ArrayList<BlockPos> var14 = Lists.newArrayList(var7);
            var13.sort(Comparator.comparingInt(Vec3i::getY));
            var14.sort(Comparator.comparingInt(Vec3i::getY));
            var5.decorators.forEach(var5x -> var5x.place(var2, var11, var3, var13, var14));
         }

         boolean var15 = BoundingBox.encapsulatingPositions(Iterables.concat(var6, var7, var8)).map(var3x -> {
            DiscreteVoxelShape var4x = updateLeaves(var2, var3x, var6, var8);
            StructureTemplate.updateShapeAtEdge(var2, 3, var4x, var3x.minX(), var3x.minY(), var3x.minZ());
            return true;
         }).orElse(false);
         McloneSchedulerTraceRecorder.recordTreeCandidateBlockCounts(var6.size(), var7.size(), var8.size(), var15);
         return var15;
      } else {
         McloneSchedulerTraceRecorder.recordTreeCandidateBlockCounts(var6.size(), var7.size(), var8.size(), false);
         return false;
      }
   }

   private static DiscreteVoxelShape updateLeaves(LevelAccessor var0, BoundingBox var1, Set<BlockPos> var2, Set<BlockPos> var3) {
      ArrayList<Set<BlockPos>> var4 = Lists.newArrayList();
      BitSetDiscreteVoxelShape var5 = new BitSetDiscreteVoxelShape(var1.getXSpan(), var1.getYSpan(), var1.getZSpan());
      byte var6 = 6;

      for (int var7 = 0; var7 < 6; var7++) {
         var4.add(Sets.newHashSet());
      }

      BlockPos.MutableBlockPos var20 = new BlockPos.MutableBlockPos();

      for (BlockPos var9 : Lists.newArrayList(var3)) {
         if (var1.isInside(var9)) {
            var5.fill(var9.getX() - var1.minX(), var9.getY() - var1.minY(), var9.getZ() - var1.minZ());
         }
      }

      for (BlockPos var23 : Lists.newArrayList(var2)) {
         if (var1.isInside(var23)) {
            var5.fill(var23.getX() - var1.minX(), var23.getY() - var1.minY(), var23.getZ() - var1.minZ());
         }

         for (Direction var13 : Direction.values()) {
            var20.setWithOffset(var23, var13);
            if (!var2.contains(var20)) {
               BlockState var14 = var0.getBlockState(var20);
               if (var14.hasProperty(BlockStateProperties.DISTANCE)) {
                  var4.get(0).add(var20.immutable());
                  setBlockKnownShape(var0, var20, var14.setValue(BlockStateProperties.DISTANCE, 1));
                  if (var1.isInside(var20)) {
                     var5.fill(var20.getX() - var1.minX(), var20.getY() - var1.minY(), var20.getZ() - var1.minZ());
                  }
               }
            }
         }
      }

      for (int var22 = 1; var22 < 6; var22++) {
         Set<BlockPos> var24 = var4.get(var22 - 1);
         Set<BlockPos> var25 = var4.get(var22);

         for (BlockPos var27 : var24) {
            if (var1.isInside(var27)) {
               var5.fill(var27.getX() - var1.minX(), var27.getY() - var1.minY(), var27.getZ() - var1.minZ());
            }

            for (Direction var16 : Direction.values()) {
               var20.setWithOffset(var27, var16);
               if (!var24.contains(var20) && !var25.contains(var20)) {
                  BlockState var17 = var0.getBlockState(var20);
                  if (var17.hasProperty(BlockStateProperties.DISTANCE)) {
                     int var18 = var17.getValue(BlockStateProperties.DISTANCE);
                     if (var18 > var22 + 1) {
                        BlockState var19 = var17.setValue(BlockStateProperties.DISTANCE, var22 + 1);
                        setBlockKnownShape(var0, var20, var19);
                        if (var1.isInside(var20)) {
                           var5.fill(var20.getX() - var1.minX(), var20.getY() - var1.minY(), var20.getZ() - var1.minZ());
                        }

                        var25.add(var20.immutable());
                     }
                  }
               }
            }
         }
      }

      return var5;
   }
}
