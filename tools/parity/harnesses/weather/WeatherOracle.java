package acacia.parity.weather;

import acacia.parity.Game;
import acacia.parity.Out;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.TreeMap;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.WeatherEffectRenderer;
import net.minecraft.client.renderer.entity.LightningBoltRenderer;
import net.minecraft.client.renderer.entity.state.LightningBoltRenderState;
import net.minecraft.client.renderer.state.level.WeatherRenderState;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.util.ARGB;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.phys.Vec3;

/**
 * Rain and snow columns and quads (WeatherEffectRenderer.extractRenderState, prepareInstances) over
 * random scenes, Biome.getPrecipitationAt and the height-adjusted temperature, lightning bolt
 * geometry (LightningBoltRenderer.submit), and rain splashes and sounds (see {@link Splashes}).
 * Args: out dir, cases per kind. Inputs go to `*_cases.jsonl`, the game's answers to `*.jsonl`.
 */
public final class WeatherOracle {
    public static void main(String[] args) throws Exception {
        Game.init();
        Path out = Path.of(args[0]);
        int cases = Integer.parseInt(args[1]);
        var biomes = new TreeMap<String, Holder<Biome>>();
        Game.world().lookupOrThrow(Registries.BIOME).listElements().forEach(h -> biomes.put(h.key().identifier().getPath(), h));
        var climate = new TreeMap<String, Object>();
        for (var e : biomes.entrySet()) {
            Object settings = Headless.get(e.getValue().value(), "climateSettings");
            var accessor = (java.util.function.Function<String, Object>) name -> {
                try {
                    Method m = settings.getClass().getDeclaredMethod(name);
                    m.setAccessible(true);
                    return m.invoke(settings);
                } catch (ReflectiveOperationException ex) {
                    throw new IllegalStateException(ex);
                }
            };
            climate.put(e.getKey(), Out.map("temperature", accessor.apply("temperature"), "has_precipitation", accessor.apply("hasPrecipitation"),
                "frozen", accessor.apply("temperatureModifier") == Biome.TemperatureModifier.FROZEN));
        }
        Out.json(out.resolve("biomes.json"), climate);
        var client = new Headless.Client();
        columns(out, cases, biomes, client);
        climate(out, cases * 10, biomes);
        bolts(out, cases);
        Splashes.write(out, cases, biomes, client);
    }

    static String bits(float v) {
        return Float.isNaN(v) ? "nan" : Integer.toHexString(Float.floatToRawIntBits(v));
    }

    static String bits(double v) {
        return Double.isNaN(v) ? "nan" : Long.toHexString(Double.doubleToRawLongBits(v));
    }

    static Vec3 camera(Random r) {
        double x = r.nextInt(4000) - 2000 + (r.nextInt(8) == 0 ? 0 : r.nextDouble());
        double y = 40 + r.nextInt(200) + r.nextDouble();
        double z = r.nextInt(4000) - 2000 + (r.nextInt(8) == 0 ? 0 : r.nextDouble());
        return new Vec3(x, y, z);
    }

    private static void columns(Path out, int cases, Map<String, Holder<Biome>> biomes, Headless.Client client) throws Exception {
        var renderer = new WeatherEffectRenderer();
        Method prepare = WeatherEffectRenderer.class.getDeclaredMethod("prepareInstances", VertexConsumer.class, List.class, Vec3.class, float.class, int.class, float.class);
        prepare.setAccessible(true);
        var level = FakeLevel.create(client);
        var r = new Random(1);
        try (var in = new Out.Lines(out.resolve("columns_cases.jsonl")); var records = new Out.Lines(out.resolve("columns.jsonl"))) {
            for (int c = 0; c < cases; c++) {
                Vec3 camera = camera(r);
                int radius = 3 + r.nextInt(8);
                float rain = c % 10 == 0 ? 0.0F : c % 3 == 0 ? 1.0F : r.nextFloat();
                float partial = r.nextFloat();
                long gameTime = c % 4 == 0 ? r.nextInt(24000) : r.nextLong() >>> 24;
                int camY = (int) Math.floor(camera.y);
                var scene = new Scene(r, (int) Math.floor(camera.x), (int) Math.floor(camera.z), radius, camY + r.nextInt(41) - 30, biomes, List.of());
                level.scene = scene;
                level.gameTime = gameTime;
                level.rain(rain);
                client.weatherRadius(radius);
                var state = new WeatherRenderState();
                renderer.extractRenderState(level, partial, camera, state);
                var vertices = new ArrayList<String>();
                VertexConsumer consumer = recorder(vertices);
                prepare.invoke(renderer, consumer, state.rainColumns, camera, 1.0F, state.radius, state.intensity);
                prepare.invoke(renderer, consumer, state.snowColumns, camera, 0.8F, state.radius, state.intensity);
                String name = "columns-" + c;
                var input = scene.json();
                input.putAll(Out.map("case", name, "camera", new double[] {camera.x, camera.y, camera.z}, "partial", partial, "game_time", gameTime,
                    "rain", rain, "radius", radius));
                in.write(input);
                records.write(Out.map("case", name, "rain", columns(state.rainColumns), "snow", columns(state.snowColumns), "vertices", vertices));
            }
        }
    }

    private static List<String> columns(List<WeatherEffectRenderer.ColumnInstance> columns) {
        var out = new ArrayList<String>();
        for (var c : columns) {
            out.add(c.x() + " " + c.z() + " " + c.bottomY() + " " + c.topY() + " " + bits(c.uOffset()) + " " + bits(c.vOffset()) + " " + c.lightCoords());
        }
        return out;
    }

    /** A VertexConsumer writing each vertex as `x y z u v color light` (float bits, colour hex). */
    static VertexConsumer recorder(List<String> vertices) {
        var current = new StringBuilder[1];
        return (VertexConsumer) Proxy.newProxyInstance(VertexConsumer.class.getClassLoader(), new Class<?>[] {VertexConsumer.class}, (proxy, method, a) -> {
            switch (method.getName()) {
                case "addVertex" -> {
                    int o = a.length == 4 ? 1 : 0;
                    current[0] = new StringBuilder(bits((Float) a[o]) + " " + bits((Float) a[o + 1]) + " " + bits((Float) a[o + 2]));
                    vertices.add("");
                }
                case "setUv" -> current[0].append(' ').append(bits((Float) a[0])).append(' ').append(bits((Float) a[1]));
                case "setColor" -> current[0].append(' ').append(Out.argb(a.length == 1 ? (Integer) a[0]
                    : ARGB.colorFromFloat((Float) a[3], (Float) a[0], (Float) a[1], (Float) a[2])));
                case "setLight" -> current[0].append(' ').append((Integer) a[0]);
                default -> throw new IllegalStateException("unexpected " + method);
            }
            vertices.set(vertices.size() - 1, current[0].toString());
            return proxy;
        });
    }

    private static void climate(Path out, int cases, Map<String, Holder<Biome>> biomes) throws Exception {
        Method adjusted = Biome.class.getDeclaredMethod("getHeightAdjustedTemperature", BlockPos.class, int.class);
        adjusted.setAccessible(true);
        var names = new ArrayList<>(biomes.keySet());
        var r = new Random(2);
        try (var in = new Out.Lines(out.resolve("climate_cases.jsonl")); var records = new Out.Lines(out.resolve("climate.jsonl"))) {
            for (int c = 0; c < cases; c++) {
                String biome = names.get(r.nextInt(names.size()));
                int spread = c % 5 == 0 ? 30_000_000 : 4000;
                var pos = new BlockPos(r.nextInt(2 * spread) - spread, -64 + r.nextInt(384), r.nextInt(2 * spread) - spread);
                Biome b = biomes.get(biome).value();
                String name = "climate-" + c;
                in.write(Out.map("case", name, "biome", biome, "pos", new int[] {pos.getX(), pos.getY(), pos.getZ()}));
                records.write(Out.map("case", name, "temperature", bits((Float) adjusted.invoke(b, pos, FakeLevel.SEA_LEVEL)),
                    "precipitation", b.getPrecipitationAt(pos, FakeLevel.SEA_LEVEL).name()));
            }
        }
    }

    private static void bolts(Path out, int cases) throws Exception {
        var renderer = Headless.allocate(LightningBoltRenderer.class);
        var r = new Random(3);
        long[] special = {0, 1, -1, Long.MIN_VALUE, Long.MAX_VALUE, 0x123456789abcdefL};
        try (var in = new Out.Lines(out.resolve("bolts_cases.jsonl")); var records = new Out.Lines(out.resolve("bolts.jsonl"))) {
            for (int c = 0; c < cases; c++) {
                long seed = c < special.length ? special[c] : r.nextLong();
                var state = new LightningBoltRenderState();
                state.seed = seed;
                var vertices = new ArrayList<String>();
                var collector = (SubmitNodeCollector) Proxy.newProxyInstance(SubmitNodeCollector.class.getClassLoader(),
                    new Class<?>[] {SubmitNodeCollector.class}, (proxy, method, a) -> {
                        if (!method.getName().equals("submitCustomGeometry")) throw new IllegalStateException("unexpected " + method);
                        ((SubmitNodeCollector.CustomGeometryRenderer) a[2]).render(((PoseStack) a[0]).last(), recorder(vertices));
                        return null;
                    });
                renderer.submit(state, new PoseStack(), collector, null);
                String name = "bolt-" + c;
                in.write(Out.map("case", name, "seed", Long.toString(seed)));
                records.write(Out.map("case", name, "vertices", vertices));
            }
        }
    }
}
