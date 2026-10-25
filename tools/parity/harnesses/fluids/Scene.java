package acacia.parity.fluids;

import java.util.Map;
import java.util.TreeMap;
import net.minecraft.client.renderer.block.BlockAndTintGetter;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.CardinalLighting;
import net.minecraft.world.level.ColorResolver;
import net.minecraft.world.level.LightLayer;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.lighting.LevelLightEngine;
import net.minecraft.world.level.material.FluidState;

/**
 * One case: listed blocks over air in section y 4 (world y 64..79), one biome everywhere, and
 * light given by position (`sky`, `block`) so a face's light tells which cells lit it.
 */
final class Scene implements BlockAndTintGetter {
    static final int MIN_Y = 64;
    final String name;
    final String biomeName;
    final Biome biome;
    final boolean nether;
    final Map<Long, BlockState> blocks = new TreeMap<>();

    Scene(String name, String biomeName, Biome biome, boolean nether) {
        this.name = name;
        this.biomeName = biomeName;
        this.biome = biome;
        this.nether = nether;
    }

    Scene put(int x, int y, int z, BlockState state) {
        blocks.put(BlockPos.asLong(x, y, z), state);
        return this;
    }

    /** Stone under x, z 1..14 at y 65. */
    Scene floor() {
        for (int x = 1; x <= 14; x++) for (int z = 1; z <= 14; z++) put(x, 65, z, Blocks.STONE.defaultBlockState());
        return this;
    }

    static int sky(int x, int y, int z) {
        return (x * 7 + y * 3 + z * 5) & 15;
    }

    static int block(int x, int y, int z) {
        return (x * 3 + y * 5 + z * 11 + 3) & 15;
    }

    @Override
    public CardinalLighting cardinalLighting() {
        return nether ? CardinalLighting.NETHER : CardinalLighting.DEFAULT;
    }

    @Override
    public LevelLightEngine getLightEngine() {
        return LevelLightEngine.EMPTY;
    }

    @Override
    public int getBrightness(LightLayer layer, BlockPos pos) {
        return layer == LightLayer.SKY ? sky(pos.getX(), pos.getY(), pos.getZ()) : block(pos.getX(), pos.getY(), pos.getZ());
    }

    @Override
    public int getBlockTint(BlockPos pos, ColorResolver resolver) {
        return 0xFF000000 | resolver.getColor(biome, pos.getX(), pos.getZ());
    }

    @Override
    public BlockEntity getBlockEntity(BlockPos pos) {
        return null;
    }

    @Override
    public BlockState getBlockState(BlockPos pos) {
        return blocks.getOrDefault(pos.asLong(), Blocks.AIR.defaultBlockState());
    }

    @Override
    public FluidState getFluidState(BlockPos pos) {
        return getBlockState(pos).getFluidState();
    }

    @Override
    public int getHeight() {
        return 384;
    }

    @Override
    public int getMinY() {
        return -64;
    }
}
