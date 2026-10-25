package acacia.parity.weather;

import acacia.parity.Out;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Random;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.particles.ParticleTypes;
import net.minecraft.server.level.ParticleStatus;
import net.minecraft.sounds.SoundEvents;
import net.minecraft.tags.FluidTags;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.CampfireBlock;
import net.minecraft.world.level.block.SlabBlock;
import net.minecraft.world.level.block.SnowLayerBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.BlockStateProperties;
import net.minecraft.world.level.material.FluidState;

/**
 * ClientLevel.tickWeatherEffects over random scenes, several ticks a case so the rain sound's
 * counter carries over: each tick's splash particles and sound. The probe gets, per column, what
 * the splash height and smoke read of the ground's top block (collision boxes, fluid height, smoke).
 */
final class Splashes {
    private static final int TICKS = 8;

    private Splashes() {}

    private static List<BlockState> palette() {
        return List.of(Blocks.STONE.defaultBlockState(), Blocks.GRASS_BLOCK.defaultBlockState(), Blocks.WATER.defaultBlockState(),
            Blocks.WATER.defaultBlockState().setValue(BlockStateProperties.LEVEL, 5), Blocks.LAVA.defaultBlockState(), Blocks.MAGMA_BLOCK.defaultBlockState(),
            Blocks.CAMPFIRE.defaultBlockState(), Blocks.CAMPFIRE.defaultBlockState().setValue(CampfireBlock.LIT, false), Blocks.SOUL_CAMPFIRE.defaultBlockState(),
            Blocks.OAK_SLAB.defaultBlockState(), Blocks.OAK_SLAB.defaultBlockState().setValue(SlabBlock.WATERLOGGED, true),
            Blocks.SNOW.defaultBlockState().setValue(SnowLayerBlock.LAYERS, 3), Blocks.FARMLAND.defaultBlockState(), Blocks.DIRT_PATH.defaultBlockState(),
            Blocks.SOUL_SAND.defaultBlockState(), Blocks.OAK_FENCE.defaultBlockState(), Blocks.HOPPER.defaultBlockState(), Blocks.CAULDRON.defaultBlockState(),
            Blocks.OAK_STAIRS.defaultBlockState(), Blocks.CHEST.defaultBlockState(), Blocks.OAK_LEAVES.defaultBlockState());
    }

    static void write(Path out, int cases, Map<String, Holder<Biome>> biomes, Headless.Client client) throws Exception {
        var palette = palette();
        var level = FakeLevel.create(client);
        var r = new Random(4);
        var statuses = ParticleStatus.values();
        try (var in = new Out.Lines(out.resolve("splashes_cases.jsonl")); var records = new Out.Lines(out.resolve("splashes.jsonl"))) {
            for (int c = 0; c < cases; c++) {
                var camera = WeatherOracle.camera(r);
                int radius = 3 + r.nextInt(8);
                float rain = c % 3 == 0 ? 1.0F : r.nextFloat();
                var status = statuses[c % 5 == 0 ? r.nextInt(statuses.length) : 0];
                long start = r.nextLong() >>> 30;
                int camY = (int) Math.floor(camera.y);
                var scene = new Scene(r, (int) Math.floor(camera.x), (int) Math.floor(camera.z), radius, camY + r.nextInt(17) - 10, biomes, palette);
                level.scene = scene;
                level.rain(rain);
                level.rainSoundTime(0);
                client.weatherRadius(radius);
                client.particles(status);
                client.camera(camera);
                var ticks = new ArrayList<Object>();
                for (int t = 0; t < TICKS; t++) {
                    level.gameTime = start + t;
                    level.particles.clear();
                    level.sounds.clear();
                    level.tickWeatherEffects();
                    var splashes = new ArrayList<String>();
                    for (Object[] p : level.particles) {
                        splashes.add((p[0] == ParticleTypes.SMOKE ? "smoke " : "rain ") + WeatherOracle.bits((Double) p[1]) + " "
                            + WeatherOracle.bits((Double) p[2]) + " " + WeatherOracle.bits((Double) p[3]));
                    }
                    String sound = null;
                    for (Object[] s : level.sounds) {
                        var pos = (BlockPos) s[0];
                        sound = (s[1] == SoundEvents.WEATHER_RAIN_ABOVE ? "above " : "rain ") + pos.getX() + " " + pos.getY() + " " + pos.getZ();
                    }
                    ticks.add(Out.map("splashes", splashes, "sound", sound));
                }
                String name = "splashes-" + c;
                var input = scene.json();
                input.putAll(Out.map("case", name, "camera", new double[] {camera.x, camera.y, camera.z}, "game_time", start, "ticks", TICKS,
                    "rain", rain, "radius", radius, "particles", status.name().toLowerCase(), "tops", tops(level, scene)));
                in.write(input);
                records.write(Out.map("case", name, "ticks", ticks));
            }
        }
    }

    /** Per column: the top block's collision boxes, its fluid's height, whether rain on it smokes. */
    private static List<Object> tops(FakeLevel level, Scene scene) {
        var out = new ArrayList<Object>();
        for (int k = 0; k < scene.side; k++) {
            for (int i = 0; i < scene.side; i++) {
                int x = scene.x0 + i, z = scene.z0 + k;
                var pos = new BlockPos(x, scene.surface(x, z) - 1, z);
                BlockState block = level.getBlockState(pos);
                FluidState fluid = level.getFluidState(pos);
                var boxes = new ArrayList<double[]>();
                for (var b : block.getCollisionShape(level, pos).toAabbs()) boxes.add(new double[] {b.minX, b.minY, b.minZ, b.maxX, b.maxY, b.maxZ});
                boolean smokes = fluid.is(FluidTags.LAVA) || block.is(Blocks.MAGMA_BLOCK) || CampfireBlock.isLitCampfire(block);
                out.add(Out.map("boxes", boxes, "fluid", fluid.getHeight(level, pos), "smokes", smokes, "state", Out.stateKey(block)));
            }
        }
        return out;
    }
}
