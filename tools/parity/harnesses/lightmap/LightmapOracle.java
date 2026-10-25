package acacia.parity.lightmap;

import acacia.parity.Out;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagLoader;
import net.minecraft.util.Mth;
import net.minecraft.world.attribute.EnvironmentAttributeSystem;
import net.minecraft.world.attribute.EnvironmentAttributes;
import net.minecraft.world.attribute.WeatherAttributes;
import net.minecraft.world.clock.ClockInstance;
import net.minecraft.world.clock.ClockManager;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.phys.Vec3;
import org.joml.Vector3fc;

/**
 * The lightmap of each case in args[1] (JSON lines: dimension, clock ticks, weather, lightning, the
 * player's options and effects) into records.jsonl: `LightmapRenderStateExtractor`'s render state
 * over vanilla's `EnvironmentAttributeSystem` (dimension, timelines, weather and the lightning
 * layer `ClientLevel` adds; no biome sets a light attribute), and the 16x16 texture `lightmap.fsh`
 * renders from it, transliterated to Java floats. Adapted from Horizon's VanillaEnvironmentOracle.
 */
public final class LightmapOracle {
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var repo = ServerPacksSource.createVanillaTrustedRepository();
        repo.reload();
        repo.setSelected(List.of("vanilla"));
        var resources = new MultiPackResourceManager(PackType.SERVER_DATA, repo.openAllSelected());
        var layers = RegistryLayer.createRegistryAccess();
        var staticTags = TagLoader.loadTagsForExistingRegistries(resources, layers.getLayer(RegistryLayer.STATIC));
        var context = TagLoader.buildUpdatedLookups(layers.getAccessForLoading(RegistryLayer.WORLD), staticTags);
        RegistryAccess.Frozen world = RegistryDataLoader.load(resources, context, RegistryDataLoader.WORLD_REGISTRIES, Runnable::run).join();
        staticTags.forEach(Registry.PendingTags::apply);
        var dimensions = world.lookupOrThrow(Registries.DIMENSION_TYPE);
        try (var out = new Out.Lines(Path.of(args[0], "records.jsonl"))) {
            for (String line : Files.readAllLines(Path.of(args[1]), StandardCharsets.UTF_8)) {
                if (line.isBlank()) continue;
                JsonObject c = JsonParser.parseString(line).getAsJsonObject();
                String dimension = c.get("dimension").getAsString();
                DimensionType dim = dimensions.getOrThrow(ResourceKey.create(Registries.DIMENSION_TYPE, Identifier.withDefaultNamespace(dimension))).value();
                out.write(record(c, system(dim, dimension, c)));
            }
        }
    }

    /** What `ClientLevel` builds: the default layers (no biome sets these) and the lightning layer. */
    private static EnvironmentAttributeSystem system(DimensionType dim, String name, JsonObject c) {
        long ticks = c.get("ticks").getAsLong();
        float rain = c.get("rain").getAsFloat(), thunder = c.get("thunder").getAsFloat();
        boolean flash = c.get("flash").getAsBoolean();
        ClockManager clocks = clock -> new ClockInstance() {
            public long totalTicks() { return ticks; }
            public float partialTick() { return 0.0F; }
            public float rate() { return 1.0F; }
            public boolean isPaused() { return false; }
        };
        var b = EnvironmentAttributeSystem.builder();
        b.addConstantLayer(dim.attributes());
        dim.timelines().forEach(t -> b.addTimelineLayer(t, clocks));
        // Level.canHaveWeather.
        if (dim.hasSkyLight() && !dim.hasCeiling() && !name.equals("the_end")) {
            WeatherAttributes.addBuiltinLayers(b, new WeatherAttributes.WeatherAccess() {
                public float rainLevel() { return rain; }
                public float thunderLevel() { return thunder; }
            });
        }
        b.addTimeBasedLayer(EnvironmentAttributes.SKY_LIGHT_FACTOR, (skyFactor, id) -> flash ? 1.0F : skyFactor);
        return b.build();
    }

    private static Object record(JsonObject c, EnvironmentAttributeSystem system) {
        float gamma = c.get("gamma").getAsFloat();
        float darknessScaleOption = c.get("darkness_scale").getAsFloat();
        float age = c.get("age").getAsFloat();
        float endFlash = c.get("end_flash").getAsFloat();
        // LightmapRenderStateExtractor.extract
        var s = new State();
        s.blockFactor = c.get("flicker").getAsFloat() + 1.4F;
        s.blockLightTint = system.getValue(EnvironmentAttributes.BLOCK_LIGHT_TINT, Vec3.ZERO, null);
        s.skyFactor = system.getValue(EnvironmentAttributes.SKY_LIGHT_FACTOR, Vec3.ZERO, null);
        s.skyLightColor = system.getValue(EnvironmentAttributes.SKY_LIGHT_COLOR, Vec3.ZERO, null);
        if (endFlash > 0.0F) {
            s.skyFactor += c.get("boss_fog").getAsBoolean() ? endFlash / 3.0F : endFlash;
        }
        s.ambientColor = system.getValue(EnvironmentAttributes.AMBIENT_LIGHT_COLOR, Vec3.ZERO, null);
        float modifier = c.get("darkness").getAsFloat() * darknessScaleOption;
        s.brightness = Math.max(0.0F, gamma - modifier);
        s.darknessEffectScale = Math.max(0.0F, Mth.cos(age * (float) Math.PI * 0.025F) * (0.45F * modifier)) * darknessScaleOption;
        s.nightVision = c.get("night_vision").getAsFloat();
        s.nightVisionColor = system.getValue(EnvironmentAttributes.NIGHT_VISION_COLOR, Vec3.ZERO, null);
        s.boss = c.get("boss").getAsFloat();
        return Out.map(
            "case", c.get("case").getAsString(),
            "sky_factor", s.skyFactor, "sky_color", list(s.skyLightColor),
            "block_factor", s.blockFactor, "block_tint", list(s.blockLightTint),
            "ambient", list(s.ambientColor),
            "night_vision", s.nightVision, "night_vision_color", list(s.nightVisionColor),
            "darkness", s.darknessEffectScale, "brightness", s.brightness, "boss", s.boss,
            "table", s.table());
    }

    private static List<Float> list(Vector3fc v) {
        return List.of(v.x(), v.y(), v.z());
    }

    /** `LightmapRenderState`, and `lightmap.fsh` over it. */
    private static final class State {
        float blockFactor, skyFactor, brightness, darknessEffectScale, nightVision, boss;
        Vector3fc blockLightTint, skyLightColor, ambientColor, nightVisionColor;

        private static float curved(float level) {
            return level / (4.0F - 3.0F * level);
        }

        private static float mix(float a, float b, float t) {
            return a * (1.0F - t) + b * t;
        }

        /** RGB bytes of the 256 texels, sky * 16 + block, as an RGBA8 target stores them. */
        List<Integer> table() {
            var out = new ArrayList<Integer>();
            for (int i = 0; i < 256; i++) {
                // A fragment's texCoord is its texel's centre, so floor(texCoord * 16) is the level.
                for (float v : texel((i % 16) / 15.0F, (i / 16) / 15.0F)) out.add(Math.round(Math.clamp(v, 0.0F, 1.0F) * 255.0F));
            }
            return out;
        }

        float[] texel(float blockLevel, float skyLevel) {
            float blockBrightness = curved(blockLevel) * blockFactor;
            float skyBrightness = curved(skyLevel) * skyFactor;
            float[] vision = {nightVisionColor.x() * nightVision, nightVisionColor.y() * nightVision, nightVisionColor.z() * nightVision};
            float[] ambient = {ambientColor.x(), ambientColor.y(), ambientColor.z()};
            float[] sky = {skyLightColor.x(), skyLightColor.y(), skyLightColor.z()};
            float[] tint = {blockLightTint.x(), blockLightTint.y(), blockLightTint.z()};
            float[] dark = {0.7F, 0.6F, 0.6F};
            float parabolic = (2.0F * blockLevel - 1.0F) * (2.0F * blockLevel - 1.0F);
            float[] color = new float[3];
            for (int k = 0; k < 3; k++) {
                float v = Math.max(ambient[k], vision[k]);
                v += sky[k] * skyBrightness;
                v += mix(tint[k], 1.0F, 0.9F * parabolic) * blockBrightness;
                v = mix(v, v * dark[k], boss);
                v = v - darknessEffectScale;
                color[k] = Math.clamp(v, 0.0F, 1.0F);
            }
            float max = Math.max(Math.max(color[0], color[1]), color[2]);
            if (max == 0.0F) return color;
            float inverted = 1.0F - max;
            float scaled = 1.0F - inverted * inverted * inverted * inverted;
            for (int k = 0; k < 3; k++) color[k] = mix(color[k], color[k] * (scaled / max), brightness);
            return color;
        }
    }
}
