package acacia.parity.weather;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.sounds.SoundSource;
import net.minecraft.world.level.LightLayer;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.material.FluidState;

/**
 * A ClientLevel built without its constructor (Headless.allocate) over a {@link Scene}: the game's
 * own weather methods run on it; only the world queries they make are answered here.
 */
final class FakeLevel extends ClientLevel {
    static final int SEA_LEVEL = 63;
    static final int MIN_Y = -64;

    Scene scene;
    long gameTime;
    final List<Object[]> particles = new ArrayList<>();
    final List<Object[]> sounds = new ArrayList<>();

    private FakeLevel() {
        super(null, null, null, null, 0, 0, null, false, 0L, 0);
    }

    static FakeLevel create(Headless.Client client) throws Exception {
        FakeLevel level = Headless.allocate(FakeLevel.class);
        Headless.set(level, "minecraft", client.minecraft);
        Headless.set(level, "particles", new ArrayList<>());
        Headless.set(level, "sounds", new ArrayList<>());
        return level;
    }

    void rain(float level) throws Exception {
        Headless.set(this, "oRainLevel", level);
        Headless.set(this, "rainLevel", level);
    }

    int rainSoundTime() throws Exception {
        return (Integer) Headless.get(this, "rainSoundTime");
    }

    void rainSoundTime(int value) throws Exception {
        Headless.set(this, "rainSoundTime", value);
    }

    @Override
    public long getGameTime() {
        return gameTime;
    }

    @Override
    public int getSeaLevel() {
        return SEA_LEVEL;
    }

    @Override
    public int getMinY() {
        return MIN_Y;
    }

    @Override
    public int getHeight(Heightmap.Types type, int x, int z) {
        return scene.loaded(x, z) ? scene.surface(x, z) : MIN_Y;
    }

    /** ClientLevel's: none outside loaded chunks, else the biome's rule. */
    @Override
    public Biome.Precipitation getPrecipitationAt(BlockPos pos) {
        if (!scene.loaded(pos.getX(), pos.getZ())) return Biome.Precipitation.NONE;
        return getBiome(pos).value().getPrecipitationAt(pos, getSeaLevel());
    }

    @Override
    public Holder<Biome> getBiome(BlockPos pos) {
        return scene.biome(pos.getX(), pos.getZ());
    }

    @Override
    public int getBrightness(LightLayer layer, BlockPos pos) {
        return scene.light(pos.getX(), pos.getZ(), layer == LightLayer.SKY);
    }

    /** Stone up to the ground, the scene's block on top, air above. */
    @Override
    public BlockState getBlockState(BlockPos pos) {
        if (!scene.loaded(pos.getX(), pos.getZ())) return Blocks.AIR.defaultBlockState();
        int surface = scene.surface(pos.getX(), pos.getZ());
        if (pos.getY() == surface - 1) return scene.top(pos.getX(), pos.getZ());
        return pos.getY() < surface - 1 ? Blocks.STONE.defaultBlockState() : Blocks.AIR.defaultBlockState();
    }

    @Override
    public FluidState getFluidState(BlockPos pos) {
        return getBlockState(pos).getFluidState();
    }

    @Override
    public void addParticle(ParticleOptions particle, double x, double y, double z, double xd, double yd, double zd) {
        particles.add(new Object[] {particle, x, y, z});
    }

    @Override
    public void playLocalSound(BlockPos pos, SoundEvent sound, SoundSource source, float volume, float pitch, boolean distanceDelay) {
        sounds.add(new Object[] {pos, sound, volume, pitch});
    }
}
