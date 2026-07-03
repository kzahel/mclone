package net.minecraft.client.renderer;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.math.Vector3f;
import net.minecraft.Util;
import net.minecraft.client.Camera;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.core.BlockPos;
import net.minecraft.util.CubicSampler;
import net.minecraft.util.Mth;
import net.minecraft.world.effect.MobEffects;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.material.FogType;
import net.minecraft.world.phys.Vec3;

// Oracle shadow: preserve vanilla 1.17.1 fog color logic, but allow screenshots
// to disable distance fog via -Dmclone.oracle.disableFog=true.
public class FogRenderer {
   private static final int WATER_FOG_DISTANCE = 192;
   public static final float BIOME_FOG_TRANSITION_TIME = 5000.0F;
   private static final String DISABLE_FOG_PROPERTY = "mclone.oracle.disableFog";
   private static float fogRed;
   private static float fogGreen;
   private static float fogBlue;
   private static int targetBiomeFog = -1;
   private static int previousBiomeFog = -1;
   private static long biomeChangedTime = -1L;

   public static void setupColor(Camera var0, float var1, ClientLevel var2, int var3, float var4) {
      FogType var5 = var0.getFluidInCamera();
      Entity var6 = var0.getEntity();
      if (var5 == FogType.WATER) {
         long var7 = Util.getMillis();
         int var9 = var2.getBiome(new BlockPos(var0.getPosition())).getWaterFogColor();
         if (biomeChangedTime < 0L) {
            targetBiomeFog = var9;
            previousBiomeFog = var9;
            biomeChangedTime = var7;
         }

         int var10 = targetBiomeFog >> 16 & 0xFF;
         int var11 = targetBiomeFog >> 8 & 0xFF;
         int var12 = targetBiomeFog & 0xFF;
         int var13 = previousBiomeFog >> 16 & 0xFF;
         int var14 = previousBiomeFog >> 8 & 0xFF;
         int var15 = previousBiomeFog & 0xFF;
         float var16 = Mth.clamp((float)(var7 - biomeChangedTime) / 5000.0F, 0.0F, 1.0F);
         float var17 = Mth.lerp(var16, (float)var13, (float)var10);
         float var18 = Mth.lerp(var16, (float)var14, (float)var11);
         float var19 = Mth.lerp(var16, (float)var15, (float)var12);
         fogRed = var17 / 255.0F;
         fogGreen = var18 / 255.0F;
         fogBlue = var19 / 255.0F;
         if (targetBiomeFog != var9) {
            targetBiomeFog = var9;
            previousBiomeFog = Mth.floor(var17) << 16 | Mth.floor(var18) << 8 | Mth.floor(var19);
            biomeChangedTime = var7;
         }
      } else if (var5 == FogType.LAVA) {
         fogRed = 0.6F;
         fogGreen = 0.1F;
         fogBlue = 0.0F;
         biomeChangedTime = -1L;
      } else if (var5 == FogType.POWDER_SNOW) {
         fogRed = 0.623F;
         fogGreen = 0.734F;
         fogBlue = 0.785F;
         biomeChangedTime = -1L;
         RenderSystem.clearColor(fogRed, fogGreen, fogBlue, 0.0F);
      } else {
         float var20 = 0.25F + 0.75F * var3 / 32.0F;
         var20 = 1.0F - (float)Math.pow(var20, 0.25);
         Vec3 var8 = var2.getSkyColor(var0.getPosition(), var1);
         float var24 = (float)var8.x;
         float var27 = (float)var8.y;
         float var29 = (float)var8.z;
         float var30 = Mth.clamp(Mth.cos(var2.getTimeOfDay(var1) * (float)(Math.PI * 2)) * 2.0F + 0.5F, 0.0F, 1.0F);
         BiomeManager var31 = var2.getBiomeManager();
         Vec3 var32 = var0.getPosition().subtract(2.0, 2.0, 2.0).scale(0.25);
         Vec3 var33 = CubicSampler.gaussianSampleVec3(
            var32,
            (var3x, var4x, var5x) -> var2.effects()
               .getBrightnessDependentFogColor(Vec3.fromRGB24(var31.getNoiseBiomeAtQuart(var3x, var4x, var5x).getFogColor()), var30)
         );
         fogRed = (float)var33.x();
         fogGreen = (float)var33.y();
         fogBlue = (float)var33.z();
         if (var3 >= 4) {
            float var34 = Mth.sin(var2.getSunAngle(var1)) > 0.0F ? -1.0F : 1.0F;
            Vector3f var36 = new Vector3f(var34, 0.0F, 0.0F);
            float var39 = var0.getLookVector().dot(var36);
            if (var39 < 0.0F) {
               var39 = 0.0F;
            }

            if (var39 > 0.0F) {
               float[] var43 = var2.effects().getSunriseColor(var2.getTimeOfDay(var1), var1);
               if (var43 != null) {
                  var39 *= var43[3];
                  fogRed = fogRed * (1.0F - var39) + var43[0] * var39;
                  fogGreen = fogGreen * (1.0F - var39) + var43[1] * var39;
                  fogBlue = fogBlue * (1.0F - var39) + var43[2] * var39;
               }
            }
         }

         fogRed = fogRed + (var24 - fogRed) * var20;
         fogGreen = fogGreen + (var27 - fogGreen) * var20;
         fogBlue = fogBlue + (var29 - fogBlue) * var20;
         float var35 = var2.getRainLevel(var1);
         if (var35 > 0.0F) {
            float var37 = 1.0F - var35 * 0.5F;
            float var41 = 1.0F - var35 * 0.4F;
            fogRed *= var37;
            fogGreen *= var37;
            fogBlue *= var41;
         }

         float var38 = var2.getThunderLevel(var1);
         if (var38 > 0.0F) {
            float var42 = 1.0F - var38 * 0.5F;
            fogRed *= var42;
            fogGreen *= var42;
            fogBlue *= var42;
         }

         biomeChangedTime = -1L;
      }

      double var22 = (var0.getPosition().y - var2.getMinBuildHeight()) * var2.getLevelData().getClearColorScale();
      if (var0.getEntity() instanceof LivingEntity && ((LivingEntity)var0.getEntity()).hasEffect(MobEffects.BLINDNESS)) {
         int var25 = ((LivingEntity)var0.getEntity()).getEffect(MobEffects.BLINDNESS).getDuration();
         if (var25 < 20) {
            var22 *= 1.0F - var25 / 20.0F;
         } else {
            var22 = 0.0;
         }
      }

      if (var22 < 1.0 && var5 != FogType.LAVA) {
         if (var22 < 0.0) {
            var22 = 0.0;
         }

         var22 *= var22;
         fogRed = (float)(fogRed * var22);
         fogGreen = (float)(fogGreen * var22);
         fogBlue = (float)(fogBlue * var22);
      }

      if (var4 > 0.0F) {
         fogRed = fogRed * (1.0F - var4) + fogRed * 0.7F * var4;
         fogGreen = fogGreen * (1.0F - var4) + fogGreen * 0.6F * var4;
         fogBlue = fogBlue * (1.0F - var4) + fogBlue * 0.6F * var4;
      }

      float var26;
      if (var5 == FogType.WATER) {
         if (var6 instanceof LocalPlayer) {
            var26 = ((LocalPlayer)var6).getWaterVision();
         } else {
            var26 = 1.0F;
         }
      } else if (var6 instanceof LivingEntity && ((LivingEntity)var6).hasEffect(MobEffects.NIGHT_VISION)) {
         var26 = GameRenderer.getNightVisionScale((LivingEntity)var6, var1);
      } else {
         var26 = 0.0F;
      }

      if (fogRed != 0.0F && fogGreen != 0.0F && fogBlue != 0.0F) {
         float var28 = Math.min(1.0F / fogRed, Math.min(1.0F / fogGreen, 1.0F / fogBlue));
         fogRed = fogRed * (1.0F - var26) + fogRed * var28 * var26;
         fogGreen = fogGreen * (1.0F - var26) + fogGreen * var28 * var26;
         fogBlue = fogBlue * (1.0F - var26) + fogBlue * var28 * var26;
      }

      RenderSystem.clearColor(fogRed, fogGreen, fogBlue, 0.0F);
   }

   public static void setupNoFog() {
      RenderSystem.setShaderFogStart(Float.MAX_VALUE);
   }

   public static void setupFog(Camera var0, FogRenderer.FogMode var1, float var2, boolean var3) {
      if (Boolean.getBoolean(DISABLE_FOG_PROPERTY)) {
         setupNoFog();
         return;
      }

      FogType var4 = var0.getFluidInCamera();
      Entity var5 = var0.getEntity();
      if (var4 == FogType.WATER) {
         float var6 = 192.0F;
         if (var5 instanceof LocalPlayer var7) {
            var6 *= Math.max(0.25F, var7.getWaterVision());
            Biome var8 = var7.level.getBiome(var7.blockPosition());
            if (var8.getBiomeCategory() == Biome.BiomeCategory.SWAMP) {
               var6 *= 0.85F;
            }
         }

         RenderSystem.setShaderFogStart(-8.0F);
         RenderSystem.setShaderFogEnd(var6 * 0.5F);
      } else {
         float var10;
         float var11;
         if (var4 == FogType.LAVA) {
            if (var5.isSpectator()) {
               var10 = -8.0F;
               var11 = var2 * 0.5F;
            } else if (var5 instanceof LivingEntity && ((LivingEntity)var5).hasEffect(MobEffects.FIRE_RESISTANCE)) {
               var10 = 0.0F;
               var11 = 3.0F;
            } else {
               var10 = 0.25F;
               var11 = 1.0F;
            }
         } else if (var5 instanceof LivingEntity && ((LivingEntity)var5).hasEffect(MobEffects.BLINDNESS)) {
            int var12 = ((LivingEntity)var5).getEffect(MobEffects.BLINDNESS).getDuration();
            float var9 = Mth.lerp(Math.min(1.0F, var12 / 20.0F), var2, 5.0F);
            if (var1 == FogRenderer.FogMode.FOG_SKY) {
               var10 = 0.0F;
               var11 = var9 * 0.8F;
            } else {
               var10 = var9 * 0.25F;
               var11 = var9;
            }
         } else if (var4 == FogType.POWDER_SNOW) {
            if (var5.isSpectator()) {
               var10 = -8.0F;
               var11 = var2 * 0.5F;
            } else {
               var10 = 0.0F;
               var11 = 2.0F;
            }
         } else if (var3) {
            var10 = var2 * 0.05F;
            var11 = Math.min(var2, 192.0F) * 0.5F;
         } else if (var1 == FogRenderer.FogMode.FOG_SKY) {
            var10 = 0.0F;
            var11 = var2;
         } else {
            var10 = var2 * 0.75F;
            var11 = var2;
         }

         RenderSystem.setShaderFogStart(var10);
         RenderSystem.setShaderFogEnd(var11);
      }
   }

   public static void levelFogColor() {
      RenderSystem.setShaderFogColor(fogRed, fogGreen, fogBlue);
   }

   public static enum FogMode {
      FOG_SKY,
      FOG_TERRAIN;
   }
}
