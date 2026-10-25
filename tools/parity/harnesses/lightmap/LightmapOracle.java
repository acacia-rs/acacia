package acacia.parity.lightmap;

import acacia.parity.Client;
import acacia.parity.Out;
import acacia.parity.Shell;
import acacia.parity.View;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.LightmapRenderStateExtractor;
import net.minecraft.client.renderer.state.LightmapRenderState;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.clock.ClockNetworkState;
import net.minecraft.world.clock.WorldClock;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.effect.MobEffects;
import org.joml.Vector3fc;

/**
 * The lightmap of each case in args[1] (JSON lines: dimension, clock ticks, weather, lightning, the
 * options and the player's effects) into records.jsonl. The state is the real client's: a fresh
 * ClientLevel with its clocks, weather and lightning set, a LocalPlayer with the effects, its camera
 * and attribute probe (View), and LightmapRenderStateExtractor.extract with the flicker and boss
 * darkening fields set. The texture is lightmap.fsh transliterated to Java floats over that state
 * (the game runs it on the GPU). Each record also carries the inputs the renderer takes, as the
 * client derives them (rain and effective thunder level, night vision scale, darkness blend factor).
 */
public final class LightmapOracle {
    public static void main(String[] args) throws Exception {
        Client.minecraft();
        try (var out = new Out.Lines(Path.of(args[0], "records.jsonl"))) {
            for (String line : Files.readAllLines(Path.of(args[1]), StandardCharsets.UTF_8)) {
                if (!line.isBlank()) out.write(record(JsonParser.parseString(line).getAsJsonObject()));
            }
        }
    }

    private static Object record(JsonObject c) throws Exception {
        var mc = Client.minecraft();
        String dimension = c.get("dimension").getAsString();
        var id = Identifier.withDefaultNamespace(dimension);
        var level = Client.level(ResourceKey.create(Registries.DIMENSION, id), ResourceKey.create(Registries.DIMENSION_TYPE, id));
        long ticks = c.get("ticks").getAsLong();
        var clocks = new HashMap<Holder<WorldClock>, ClockNetworkState>();
        level.registryAccess().lookupOrThrow(Registries.WORLD_CLOCK).listElements().forEach(h -> clocks.put(h, new ClockNetworkState(ticks, 0.0F, 1.0F)));
        level.clockManager().handleUpdates(0L, clocks);
        level.setRainLevel(c.get("rain").getAsFloat());
        level.setThunderLevel(c.get("thunder").getAsFloat());
        if (c.get("flash").getAsBoolean()) level.setSkyFlashTime(2);
        var flash = level.endFlashState();
        if (flash != null) {
            Shell.set(flash, "intensity", c.get("end_flash").getAsFloat());
            Shell.set(flash, "oldIntensity", c.get("end_flash").getAsFloat());
        }
        mc.options.gamma().set((double) c.get("gamma").getAsFloat());
        mc.options.darknessEffectScale().set((double) c.get("darkness_scale").getAsFloat());
        var player = View.player(level, 8.5, 64, 8.5, 0.0F, 0.0F);
        player.tickCount = c.get("age").getAsInt();
        int nightVision = c.get("night_vision_ticks").getAsInt();
        if (nightVision > 0) player.addEffect(new MobEffectInstance(MobEffects.NIGHT_VISION, nightVision));
        int darkness = c.get("darkness_ticks").getAsInt();
        if (darkness >= 0) {
            player.addEffect(new MobEffectInstance(MobEffects.DARKNESS, 1000));
            for (int i = 0; i < darkness; i++) player.getEffect(MobEffects.DARKNESS).tickClient();
        }
        var renderer = View.gameRenderer();
        Shell.set(renderer, "bossOverlayWorldDarkening", c.get("boss").getAsFloat());
        Shell.set(renderer, "bossOverlayWorldDarkeningO", c.get("boss").getAsFloat());
        var camera = View.camera(0.0F);
        // The attribute probe blends its last two ticks' values; two ticks make both this case's.
        camera.tick();
        var extractor = new LightmapRenderStateExtractor(renderer, mc);
        Shell.set(extractor, "needsUpdate", true);
        Shell.set(extractor, "blockLightFlicker", c.get("flicker").getAsFloat());
        var s = new LightmapRenderState();
        extractor.extract(s, 0.0F);
        float vision = player.hasEffect(MobEffects.NIGHT_VISION) ? GameRenderer.nightVisionScale(player, 0.0F) : 0.0F;
        return Out.map(
            "case", c.get("case").getAsString(), "dimension", dimension, "ticks", ticks,
            "rain", level.getRainLevel(0.0F), "thunder", level.getThunderLevel(0.0F), "flash", c.get("flash").getAsBoolean(),
            "gamma", c.get("gamma").getAsFloat(), "night_vision", vision,
            "darkness_blend", player.getEffectBlendFactor(MobEffects.DARKNESS, 0.0F), "darkness_scale", c.get("darkness_scale").getAsFloat(),
            "age", (float) player.tickCount, "boss", c.get("boss").getAsFloat(), "end_flash", flash == null ? 0.0F : flash.getIntensity(0.0F),
            "flicker", c.get("flicker").getAsFloat(),
            "state", Out.map("sky_factor", s.skyFactor, "sky_color", list(s.skyLightColor), "block_factor", s.blockFactor,
                "block_tint", list(s.blockLightTint), "ambient", list(s.ambientColor), "night_vision", s.nightVisionEffectIntensity,
                "night_vision_color", list(s.nightVisionColor), "darkness", s.darknessEffectScale, "brightness", s.brightness,
                "boss", s.bossOverlayWorldDarkening, "table", table(s)));
    }

    private static List<Float> list(Vector3fc v) {
        return List.of(v.x(), v.y(), v.z());
    }

    private static float curved(float level) {
        return level / (4.0F - 3.0F * level);
    }

    private static float mix(float a, float b, float t) {
        return a * (1.0F - t) + b * t;
    }

    /** RGB bytes of the 256 texels, sky * 16 + block, as the RGBA8 target stores them. */
    private static List<Integer> table(LightmapRenderState s) {
        var out = new ArrayList<Integer>();
        for (int i = 0; i < 256; i++) {
            // A fragment's texCoord is its texel's centre, so floor(texCoord * 16) is the level.
            for (float v : texel(s, (i % 16) / 15.0F, (i / 16) / 15.0F)) out.add(Math.round(Math.clamp(v, 0.0F, 1.0F) * 255.0F));
        }
        return out;
    }

    /** lightmap.fsh's main for one texel. */
    private static float[] texel(LightmapRenderState s, float blockLevel, float skyLevel) {
        float blockBrightness = curved(blockLevel) * s.blockFactor;
        float skyBrightness = curved(skyLevel) * s.skyFactor;
        var nv = s.nightVisionColor;
        float[] vision = {nv.x() * s.nightVisionEffectIntensity, nv.y() * s.nightVisionEffectIntensity, nv.z() * s.nightVisionEffectIntensity};
        float[] ambient = {s.ambientColor.x(), s.ambientColor.y(), s.ambientColor.z()};
        float[] sky = {s.skyLightColor.x(), s.skyLightColor.y(), s.skyLightColor.z()};
        float[] tint = {s.blockLightTint.x(), s.blockLightTint.y(), s.blockLightTint.z()};
        float[] dark = {0.7F, 0.6F, 0.6F};
        float parabolic = (2.0F * blockLevel - 1.0F) * (2.0F * blockLevel - 1.0F);
        float[] color = new float[3];
        for (int k = 0; k < 3; k++) {
            float v = Math.max(ambient[k], vision[k]);
            v += sky[k] * skyBrightness;
            v += mix(tint[k], 1.0F, 0.9F * parabolic) * blockBrightness;
            v = mix(v, v * dark[k], s.bossOverlayWorldDarkening);
            v = v - s.darknessEffectScale;
            color[k] = Math.clamp(v, 0.0F, 1.0F);
        }
        float max = Math.max(Math.max(color[0], color[1]), color[2]);
        if (max == 0.0F) return color;
        float inverted = 1.0F - max;
        float scaled = 1.0F - inverted * inverted * inverted * inverted;
        for (int k = 0; k < 3; k++) color[k] = mix(color[k], color[k] * (scaled / max), s.brightness);
        return color;
    }
}
