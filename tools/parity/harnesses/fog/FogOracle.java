package acacia.parity.fog;

import acacia.parity.Environments;
import acacia.parity.Game;
import acacia.parity.Headless;
import acacia.parity.Out;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.client.Camera;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.renderer.fog.FogRenderer;
import net.minecraft.client.renderer.fog.environment.AtmosphericFogEnvironment;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.tags.FluidTags;
import net.minecraft.tags.TagKey;
import net.minecraft.world.attribute.EnvironmentAttribute;
import net.minecraft.world.attribute.EnvironmentAttributeProbe;
import net.minecraft.world.attribute.SpatialAttributeInterpolator;
import net.minecraft.world.effect.MobEffect;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.effect.MobEffects;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.material.Fluid;
import net.minecraft.world.level.material.FogType;
import net.minecraft.world.phys.Vec3;
import org.joml.Vector3f;
import org.joml.Vector3fc;

/** FogRenderer.setupFog (and its computeFogColor) over FogCases' cameras. Writes cases.jsonl and records.jsonl. */
public final class FogOracle {
    private FogOracle() {}

    public static void main(String[] args) throws Exception {
        Game.init();
        Path out = Path.of(args[0]);
        var renderer = Headless.allocate(FogRenderer.class);
        try (var cases = new Out.Lines(out.resolve("cases.jsonl")); var records = new Out.Lines(out.resolve("records.jsonl"))) {
            for (var c : FogCases.all()) {
                // The world's (server's) biome data, not the fog's rule: an input.
                c.put("has_precipitation", Environments.biome((String) c.get("biome")).value().hasPrecipitation());
                cases.write(c);
                records.write(fog(renderer, c));
            }
        }
    }

    /** The level's attributes at the camera, its one biome unblended. */
    static final class Probe extends EnvironmentAttributeProbe {
        Environments.FakeLevel level;
        Vec3 position;
        SpatialAttributeInterpolator biomes;

        @Override
        public <Value> Value getValue(final EnvironmentAttribute<Value> attribute, final float partialTicks) {
            return level.system.getValue(attribute, position, biomes);
        }
    }

    static final class FakeCamera extends Camera {
        FogType fluid;
        Entity entity;
        Vec3 position;
        Vector3fc forward;
        Probe probe;

        @Override public FogType getFluidInCamera() { return fluid; }
        @Override public Entity entity() { return entity; }
        @Override public Vec3 position() { return position; }
        @Override public BlockPos blockPosition() { return BlockPos.containing(position); }
        @Override public Vector3fc forwardVector() { return forward; }
        @Override public boolean isPanoramicMode() { return false; }
        @Override public EnvironmentAttributeProbe attributeProbe() { return probe; }
    }

    static final class FakePlayer extends LocalPlayer {
        Map<Holder<MobEffect>, MobEffectInstance> effects;
        boolean spectator, eyeInWater;

        private FakePlayer() {
            super(null, null, null, null, null, null, false, null, null);
        }

        @Override public boolean isSpectator() { return spectator; }
        @Override public boolean hasEffect(final Holder<MobEffect> effect) { return effects.containsKey(effect); }
        @Override public MobEffectInstance getEffect(final Holder<MobEffect> effect) { return effects.get(effect); }
        @Override public boolean isEyeInFluid(final TagKey<Fluid> fluid) { return eyeInWater && fluid == FluidTags.WATER; }
    }

    private static float number(Map<String, Object> c, String key) {
        return ((Number) c.get(key)).floatValue();
    }

    private static MobEffectInstance effect(Holder<MobEffect> effect, int duration) {
        return new MobEffectInstance(effect, duration);
    }

    @SuppressWarnings("unchecked")
    private static Map<String, Object> fog(FogRenderer renderer, Map<String, Object> c) {
        Environments.installMinecraft(((Number) c.get("cloud_range")).intValue()).worldFog = (Boolean) c.get("boss_fog");
        var biome = Environments.biome((String) c.get("biome"));
        var level = Environments.level((String) c.get("dimension"), biome, ((Number) c.get("ticks")).longValue(), number(c, "rain"), number(c, "thunder"),
            false, (Boolean) c.get("flat"));
        level.skyLight = ((Number) c.get("sky_light")).intValue();
        level.minY = ((Number) c.get("min_y")).intValue();

        var fluid = switch ((String) c.get("fog_type")) {
            case "water" -> FogType.WATER;
            case "lava" -> FogType.LAVA;
            case "powder_snow" -> FogType.POWDER_SNOW;
            default -> FogType.NONE;
        };
        var player = Headless.allocate(FakePlayer.class);
        player.effects = new HashMap<>();
        player.spectator = (Boolean) c.get("spectator");
        player.eyeInWater = fluid == FogType.WATER;
        Headless.set(player, "waterVisionTime", ((Number) c.get("water_vision_time")).intValue());
        if ((Boolean) c.get("fire_resistance")) player.effects.put(MobEffects.FIRE_RESISTANCE, effect(MobEffects.FIRE_RESISTANCE, -1));
        if (c.get("blindness") instanceof Number d) player.effects.put(MobEffects.BLINDNESS, effect(MobEffects.BLINDNESS, d.intValue()));
        if (c.get("night_vision") instanceof Number d) player.effects.put(MobEffects.NIGHT_VISION, effect(MobEffects.NIGHT_VISION, d.intValue()));
        if (c.get("darkness") instanceof List<?> blend) {
            var darkness = effect(MobEffects.DARKNESS, -1);
            var state = Headless.get(darkness, "blendState");
            Headless.set(state, "factor", ((Number) blend.get(0)).floatValue());
            Headless.set(state, "factorPreviousFrame", ((Number) blend.get(1)).floatValue());
            player.effects.put(MobEffects.DARKNESS, darkness);
        }

        var forward = (List<Number>) c.get("forward");
        var camera = Headless.allocate(FakeCamera.class);
        camera.fluid = fluid;
        camera.entity = player;
        camera.position = new Vec3(0.5, ((Number) c.get("camera_y")).doubleValue(), 0.5);
        camera.forward = new Vector3f(forward.get(0).floatValue(), forward.get(1).floatValue(), forward.get(2).floatValue());
        camera.probe = Headless.allocate(Probe.class);
        camera.probe.level = level;
        camera.probe.position = camera.position;
        camera.probe.biomes = Environments.blend(List.of(Map.entry(biome, 1.0)));

        var atmospheric = ((List<?>) Headless.getStatic(FogRenderer.class, "FOG_ENVIRONMENTS")).stream()
            .filter(AtmosphericFogEnvironment.class::isInstance).findFirst().orElseThrow();
        Headless.set(atmospheric, "rainFogMultiplier", number(c, "rain_fog_before"));
        float delta = number(c, "delta_ticks"), partial = number(c, "partial");
        var tracker = new DeltaTracker() {
            @Override public float getGameTimeDeltaTicks() { return delta; }
            @Override public float getGameTimeDeltaPartialTick(final boolean ignoreFrozenGame) { return partial; }
            @Override public float getRealtimeDeltaTicks() { return delta; }
        };
        var fog = renderer.setupFog(camera, ((Number) c.get("render_distance")).intValue(), tracker, number(c, "darken_world"), (ClientLevel) level);
        return Out.map("case", c.get("case"), "color", List.of(fog.color.x, fog.color.y, fog.color.z),
            "environmental_start", fog.environmentalStart, "environmental_end", fog.environmentalEnd,
            "render_distance_start", fog.renderDistanceStart, "render_distance_end", fog.renderDistanceEnd,
            "sky_end", fog.skyEnd, "cloud_end", fog.cloudEnd);
    }
}
