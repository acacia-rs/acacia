package acacia.parity.sky;

import acacia.parity.Atlases;
import acacia.parity.Environments;
import acacia.parity.Game;
import acacia.parity.Headless;
import acacia.parity.Out;
import com.mojang.blaze3d.vertex.BufferBuilder;
import com.mojang.blaze3d.vertex.ByteBufferBuilder;
import com.mojang.blaze3d.vertex.DefaultVertexFormat;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import com.mojang.math.Axis;
import com.mojang.renderpearl.api.pipeline.PrimitiveTopology;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import net.minecraft.client.renderer.SkyRenderer;
import net.minecraft.client.renderer.texture.TextureAtlas;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.data.AtlasIds;
import net.minecraft.util.Mth;
import net.minecraft.world.attribute.EnvironmentAttribute;
import net.minecraft.world.attribute.GaussianSampler;
import net.minecraft.world.attribute.SpatialAttributeInterpolator;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.phys.Vec3;
import org.joml.Matrix4f;

/**
 * Every rendered environment attribute over Horizon's case grid (each dimension's times, weather
 * and biomes) plus lightning flashes, weighted biome blends and GaussianSampler blends; and
 * SkyRenderer's vertex buffers (recorded from the GPU device) and its draw-time frames.
 * Writes cases.jsonl (the inputs) and records.jsonl.
 */
public final class SkyOracle {
    private static final List<String> NETHER = List.of("nether_wastes", "soul_sand_valley", "crimson_forest", "warped_forest", "basalt_deltas");
    private static final List<String> END = List.of("the_end", "small_end_islands", "end_midlands", "end_highlands", "end_barrens");
    private static final List<String> PALETTE = List.of("plains", "swamp", "desert", "mangrove_swamp", "cherry_grove");

    private SkyOracle() {}

    public static void main(String[] args) throws Exception {
        Game.init();
        Path out = Path.of(args[0]);
        try (var cases = new Out.Lines(out.resolve("cases.jsonl")); var records = new Out.Lines(out.resolve("records.jsonl"))) {
            for (var c : attributeCases()) {
                cases.write(c);
                records.write(attributes(c));
            }
            geometry(cases, records);
        }
    }

    private static Map<String, Object> attributeCase(String dimension, long ticks, float rain, float thunder, boolean flash, List<List<Object>> biomes) {
        var key = dimension + " " + ticks + " " + rain + " " + thunder + (flash ? " flash" : "") + " " + biomes;
        return Out.map("case", key, "kind", "attributes", "dimension", dimension, "ticks", ticks, "rain", rain, "thunder", thunder, "flash", flash,
            "biomes", biomes);
    }

    private static List<Map<String, Object>> attributeCases() {
        var out = new ArrayList<Map<String, Object>>();
        float[][] overworldWeather = {{0, 0}, {0.5F, 0}, {1, 0}, {1, 1}, {0.7F, 0.3F}};
        float[][] still = {{0, 0}};
        for (var dimension : List.of("overworld", "the_nether", "the_end")) {
            var weathers = dimension.equals("overworld") ? overworldWeather : still;
            var ticks = new ArrayList<Long>();
            for (long t = 0; t < 48000; t += 250) ticks.add(t);
            ticks.addAll(List.of(23999L, 24001L, 1234567L));
            for (long t : ticks) for (var w : weathers) out.add(attributeCase(dimension, t, w[0], w[1], false, List.of(List.of("plains", 1.0))));
        }
        for (var dimension : List.of("overworld", "the_nether", "the_end")) {
            for (var biome : Environments.biomes()) {
                boolean nether = NETHER.contains(biome), end = END.contains(biome);
                if (dimension.equals("the_nether") ? !nether : dimension.equals("the_end") ? !end : nether || end) continue;
                float[][] weathers = dimension.equals("overworld") ? new float[][] {{0, 0}, {1, 0.4F}} : still;
                for (long t : List.of(0L, 6000L, 12500L, 18000L)) {
                    for (var w : weathers) out.add(attributeCase(dimension, t, w[0], w[1], false, List.of(List.of(biome, 1.0))));
                }
            }
        }
        for (long t : List.of(6000L, 13000L, 18000L)) {
            for (float rain : new float[] {0, 1}) out.add(attributeCase("overworld", t, rain, rain * 0.5F, true, List.of(List.of("plains", 1.0))));
        }
        List<List<List<Object>>> blends = List.of(
            List.of(List.of("plains", 0.3), List.of("swamp", 0.7)),
            List.of(List.of("swamp", 2.5), List.of("desert", 1.0), List.of("mangrove_swamp", 0.25)),
            List.of(List.of("desert", 1.0), List.of("badlands", 1.0), List.of("plains", 3.0)),
            List.of(List.of("cherry_grove", 0.1), List.of("pale_garden", 0.9)));
        for (var blend : blends) {
            for (long t : List.of(0L, 13000L)) for (float rain : new float[] {0, 1}) out.add(attributeCase("overworld", t, rain, 0, false, blend));
        }
        out.add(attributeCase("the_nether", 0, 0, 0, false, List.of(List.of("crimson_forest", 0.5), List.of("warped_forest", 0.25), List.of("basalt_deltas", 0.25))));
        out.add(attributeCase("the_end", 0, 0, 0, false, List.of(List.of("the_end", 0.6), List.of("small_end_islands", 0.4))));
        double[][] positions = {{0, 64, 0}, {1.3, 70.2, -5.7}, {100.5, 64, 33.25}, {-7.9, 12.1, 4.4}, {2.0, 2.0, 2.0}};
        for (var p : positions) {
            for (long t : List.of(6000L, 13000L)) {
                var key = "gauss " + p[0] + " " + p[1] + " " + p[2] + " " + t;
                out.add(Out.map("case", key, "kind", "gauss", "dimension", "overworld", "ticks", t, "rain", 0.0F, "thunder", 0.0F, "flash", false,
                    "position", List.of(p[0], p[1], p[2]), "palette", PALETTE));
            }
        }
        return out;
    }

    /** The probe's stand-in world: quart cell (x, y, z) holds palette[floorMod(x + 2z + 3y, n)]. */
    private static Holder<Biome> paletteBiome(int x, int y, int z) {
        return Environments.biome(PALETTE.get(Math.floorMod(x + 2 * z + 3 * y, PALETTE.size())));
    }

    @SuppressWarnings("unchecked")
    private static Map<String, Object> attributes(Map<String, Object> c) {
        SpatialAttributeInterpolator biomes;
        Holder<Biome> camera;
        if (c.get("kind").equals("gauss")) {
            var p = (List<Double>) c.get("position");
            camera = paletteBiome(Mth.floor(p.get(0)) >> 2, Mth.floor(p.get(1)) >> 2, Mth.floor(p.get(2)) >> 2);
            biomes = new SpatialAttributeInterpolator();
            var interpolator = biomes;
            GaussianSampler.sample(new Vec3(p.get(0), p.get(1), p.get(2)).scale(0.25), SkyOracle::paletteBiome,
                (weight, biome) -> interpolator.accumulate(weight, biome.value().getAttributes()));
        } else {
            var weights = new ArrayList<Map.Entry<Holder<Biome>, Double>>();
            for (var b : (List<List<Object>>) c.get("biomes")) weights.add(Map.entry(Environments.biome((String) b.get(0)), (Double) b.get(1)));
            biomes = Environments.blend(weights);
            camera = weights.get(0).getKey();
        }
        var level = Environments.level((String) c.get("dimension"), camera, (Long) c.get("ticks"), (Float) c.get("rain"), (Float) c.get("thunder"),
            (Boolean) c.get("flash"), false);
        var record = Out.map("case", c.get("case"));
        for (var entry : BuiltInRegistries.ENVIRONMENT_ATTRIBUTE.entrySet()) {
            var path = entry.getKey().identifier().getPath();
            if (!path.startsWith("visual/") && !path.equals("gameplay/sky_light_level")) continue;
            var value = Environments.json(Environments.value(level, (EnvironmentAttribute<?>) entry.getValue(), biomes));
            if (value != null) record.put(entry.getKey().identifier().toString(), value);
        }
        return record;
    }

    /** One vertex buffer's vertices: each `stride` bytes, positions then uvs at `uv` (or -1), RGBA bytes at `color` (or -1). */
    private static Map<String, Object> vertices(String name, ByteBuffer data, int stride, int uv, int color) {
        var b = data.duplicate().order(ByteOrder.LITTLE_ENDIAN);
        var positions = new ArrayList<Float>();
        var uvs = new ArrayList<Float>();
        var colors = new ArrayList<Integer>();
        for (int at = 0; at + stride <= b.limit(); at += stride) {
            for (int i = 0; i < 3; i++) positions.add(b.getFloat(at + 4 * i));
            if (uv >= 0) for (int i = 0; i < 2; i++) uvs.add(b.getFloat(at + uv + 4 * i));
            if (color >= 0) for (int i = 0; i < 4; i++) colors.add(b.get(at + color + i) & 255);
        }
        var record = Out.map("case", "geometry/" + name, "kind", "geometry", "positions", positions);
        if (uv >= 0) record.put("uvs", uvs);
        if (color >= 0) record.put("colors", colors);
        return record;
    }

    private static float[] matrix(Matrix4f m) {
        return m.get(new float[16]);
    }

    private static void geometry(Out.Lines cases, Out.Lines records) throws Exception {
        var buffers = Headless.recordGpuBuffers();
        // applyCelestialBodyTransform goes through RenderSystem's (identity) model-view stack.
        Headless.setStatic(Class.forName("com.mojang.blaze3d.systems.RenderSystem"), "renderThread", Thread.currentThread());
        var sky = Headless.allocate(SkyRenderer.class);
        var none = new Class<?>[0];
        Headless.call(SkyRenderer.class, sky, "buildStars", none);
        Headless.call(SkyRenderer.class, null, "buildEndSky", none);
        Headless.call(SkyRenderer.class, sky, "buildSunriseFan", none);
        var atlas = Headless.allocate(TextureAtlas.class);
        var sprites = Atlases.prepared(AtlasIds.CELESTIALS);
        Headless.set(atlas, "texturesByName", sprites.regions());
        Headless.set(atlas, "missingSprite", sprites.missing());
        Headless.call(SkyRenderer.class, null, "buildSunQuad", new Class<?>[] {TextureAtlas.class}, atlas);
        Headless.call(SkyRenderer.class, null, "buildMoonPhases", new Class<?>[] {TextureAtlas.class}, atlas);
        // The constructor's two buildSkyDisc buffers.
        for (float y : new float[] {16.0F, -16.0F}) {
            try (var bytes = ByteBufferBuilder.exactlySized(10 * DefaultVertexFormat.POSITION.getVertexSize())) {
                var builder = new BufferBuilder(bytes, PrimitiveTopology.TRIANGLE_FAN, DefaultVertexFormat.POSITION);
                Headless.call(SkyRenderer.class, sky, "buildSkyDisc", new Class<?>[] {VertexConsumer.class, float.class}, builder, y);
                try (var mesh = builder.buildOrThrow()) {
                    var copy = ByteBuffer.allocate(mesh.vertexBuffer().remaining());
                    copy.put(mesh.vertexBuffer().duplicate()).flip();
                    buffers.put(y > 0 ? "sky disc" : "dark disc", copy);
                }
            }
        }
        var written = List.of(
            vertices("sky_disc", buffers.get("sky disc"), 12, -1, -1),
            vertices("dark_disc", buffers.get("dark disc"), 12, -1, -1),
            vertices("stars", buffers.get("Stars vertex buffer"), 12, -1, -1),
            vertices("sunrise_fan", buffers.get("Sunrise/Sunset fan"), 16, -1, 12),
            vertices("sun", buffers.get("Sun quad"), 20, 12, -1),
            vertices("moon", buffers.get("Moon phases"), 20, 12, -1),
            vertices("end_sky", buffers.get("End sky vertex buffer"), 24, 12, 20));
        for (var r : written) {
            cases.write(Out.map("case", r.get("case"), "kind", "geometry"));
            records.write(r);
        }
        for (float angle : new float[] {0, 30, 89.5F, 90, 135, 180, 200, 270, 300, 359.9F}) {
            for (var body : List.of("sun", "moon", "stars")) {
                var pose = new PoseStack();
                pose.rotateDegrees(Axis.YP, -90.0F);
                pose.rotate(Axis.XP, angle * (float) (Math.PI / 180.0));
                var m = body.equals("stars") ? new Matrix4f(pose.last().pose())
                    : (Matrix4f) Headless.call(SkyRenderer.class, sky, "applyCelestialBodyTransform", new Class<?>[] {PoseStack.class, float.class, float.class},
                        pose, 100.0F, body.equals("sun") ? 30.0F : 20.0F);
                var key = "frame/" + body + " " + angle;
                cases.write(Out.map("case", key, "kind", "frame", "body", body, "angle", angle));
                records.write(Out.map("case", key, "matrix", matrix(m)));
            }
            for (float alpha : new float[] {0.25F, 1.0F}) {
                // renderSunriseAndSunset's pose.
                var pose = new PoseStack();
                pose.rotateDegrees(Axis.XP, 90.0F);
                float flip = Mth.sin(angle * (float) (Math.PI / 180.0)) < 0.0F ? 180.0F : 0.0F;
                pose.rotateDegrees(Axis.ZP, flip + 90.0F);
                var m = new Matrix4f(pose.last().pose()).scale(1.0F, 1.0F, alpha);
                var key = "frame/sunrise " + angle + " " + alpha;
                cases.write(Out.map("case", key, "kind", "frame", "body", "sunrise", "angle", angle, "alpha", alpha));
                records.write(Out.map("case", key, "matrix", matrix(m)));
            }
        }
        cases.write(Out.map("case", "frame/dark_disc", "kind", "frame", "body", "dark_disc"));
        records.write(Out.map("case", "frame/dark_disc", "matrix", matrix(new Matrix4f().translate(0.0F, 12.0F, 0.0F))));
    }
}
