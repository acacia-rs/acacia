package acacia.parity.fog;

import acacia.parity.Out;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

/** The fog vocabulary: each fog type, the effects and boss bars that change it, rain fog, void darkness, render and cloud distance. */
final class FogCases {
    private static final List<List<Float>> FORWARDS = List.of(List.of(1F, 0F, 0F), List.of(-1F, 0F, 0F), List.of(0F, 0F, 1F), List.of(0.6F, 0.2F, 0.774597F));

    private final List<Map<String, Object>> out = new ArrayList<>();

    private FogCases() {}

    /** An atmospheric overworld camera at noon in plains, nothing on it; `with` overrides. */
    private static TreeMap<String, Object> base() {
        return Out.map("fog_type", "atmospheric", "dimension", "overworld", "biome", "plains", "ticks", 6000L, "rain", 0F, "thunder", 0F,
            "render_distance", 12, "cloud_range", 128, "camera_y", 70.0, "min_y", -64, "flat", false, "forward", FORWARDS.get(0),
            "sky_light", 15, "rain_fog_before", 0F, "delta_ticks", 1F, "partial", 1F, "spectator", false, "fire_resistance", false,
            "darken_world", 0F, "boss_fog", false, "water_vision_time", 600);
    }

    private void add(Object... overrides) {
        var c = base();
        c.putAll(Out.map(overrides));
        var key = new StringBuilder("fog");
        for (int i = 0; i < overrides.length; i += 2) key.append(' ').append(overrides[i]).append('=').append(overrides[i + 1]);
        c.put("case", out.size() + " " + key);
        out.add(c);
    }

    static List<Map<String, Object>> all() {
        var cases = new FogCases();
        cases.atmospheric();
        cases.fluids();
        cases.effects();
        return cases.out;
    }

    private void atmospheric() {
        Map<String, List<String>> biomes = Map.of(
            "overworld", List.of("plains", "desert", "swamp", "snowy_plains", "pale_garden"),
            "the_nether", List.of("nether_wastes", "basalt_deltas", "soul_sand_valley"),
            "the_end", List.of("the_end"));
        for (var dimension : List.of("overworld", "the_nether", "the_end")) {
            boolean overworld = dimension.equals("overworld");
            for (var biome : biomes.get(dimension)) {
                for (long ticks : overworld ? List.of(0L, 6000L, 12000L, 12800L, 13000L, 18000L, 23200L) : List.of(0L)) {
                    for (var forward : FORWARDS) {
                        for (int rd : List.of(2, 4, 12, 32)) {
                            add("dimension", dimension, "biome", biome, "ticks", ticks, "forward", forward, "render_distance", rd);
                            if (overworld) add("dimension", dimension, "biome", biome, "ticks", ticks, "forward", forward, "render_distance", rd,
                                "rain", 1F, "thunder", 0.5F);
                        }
                    }
                }
            }
        }
        for (var biome : List.of("plains", "desert")) {
            for (float before : new float[] {0F, 0.4F}) {
                for (int light : List.of(15, 12, 8, 4)) {
                    for (float delta : new float[] {1F, 0.5F, 3F}) {
                        add("biome", biome, "rain", 0.7F, "rain_fog_before", before, "sky_light", light, "delta_ticks", delta);
                    }
                }
            }
        }
        for (double y : new double[] {-90, -64, -50, -32, 0, 70}) {
            for (boolean flat : new boolean[] {false, true}) add("camera_y", y, "flat", flat, "ticks", 18000L);
        }
        for (int clouds : List.of(2, 8, 64, 128)) add("cloud_range", clouds, "render_distance", 32);
        for (float darken : new float[] {0.3F, 1F}) add("darken_world", darken);
        add("boss_fog", true);
        add("boss_fog", true, "rain", 1F, "rain_fog_before", 1F);
        add("boss_fog", true, "darken_world", 0.5F, "dimension", "the_end", "biome", "the_end", "ticks", 0L);
    }

    private void fluids() {
        for (var biome : List.of("plains", "swamp", "warm_ocean", "cold_ocean", "lukewarm_ocean")) {
            for (int vision : List.of(0, 50, 100, 300, 599, 600)) add("fog_type", "water", "biome", biome, "water_vision_time", vision);
        }
        add("fog_type", "water", "camera_y", -80.0);
        add("fog_type", "water", "night_vision", -1);
        add("fog_type", "water", "blindness", 10);
        add("fog_type", "water", "darkness", List.of(0.5F, 0.25F));
        for (var type : List.of("lava", "powder_snow")) {
            for (int rd : List.of(2, 12)) {
                add("fog_type", type, "render_distance", rd);
                add("fog_type", type, "render_distance", rd, "spectator", true);
                add("fog_type", type, "render_distance", rd, "fire_resistance", true);
            }
            add("fog_type", type, "camera_y", -80.0);
            add("fog_type", type, "blindness", -1);
            add("fog_type", type, "darken_world", 1F);
            add("fog_type", type, "night_vision", -1);
        }
    }

    private void effects() {
        for (var type : List.of("atmospheric", "water")) {
            for (int duration : List.of(-1, 1, 5, 19, 20, 21, 100)) {
                add("fog_type", type, "blindness", duration);
                add("fog_type", type, "blindness", duration, "darkness", List.of(1F, 1F));
            }
            for (var blend : List.of(List.of(0.5F, 0.3F), List.of(1F, 1F), List.of(0F, 0.2F))) {
                for (float partial : new float[] {0.25F, 1F}) add("fog_type", type, "darkness", blend, "partial", partial);
            }
        }
        for (int duration : List.of(-1, 1, 100, 199, 200, 201, 1000)) {
            for (float partial : new float[] {0F, 0.5F, 1F}) add("night_vision", duration, "partial", partial, "ticks", 18000L);
        }
        add("night_vision", -1, "darkness", List.of(0.5F, 0.5F));
        add("night_vision", -1, "darken_world", 0.5F, "ticks", 13000L);
    }
}
