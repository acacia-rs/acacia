package acacia.parity;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import net.minecraft.client.ClientClockManager;
import net.minecraft.client.Minecraft;
import net.minecraft.client.OptionInstance;
import net.minecraft.client.Options;
import net.minecraft.client.gui.Gui;
import net.minecraft.client.gui.Hud;
import net.minecraft.client.gui.components.BossHealthOverlay;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.extract.LevelExtractor;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagLoader;
import net.minecraft.world.attribute.EnvironmentAttribute;
import net.minecraft.world.attribute.EnvironmentAttributeSystem;
import net.minecraft.world.attribute.SpatialAttributeInterpolator;
import net.minecraft.world.clock.WorldClock;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.LightLayer;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.lighting.LayerLightEventListener;
import net.minecraft.world.level.lighting.LevelLightEngine;
import net.minecraft.world.phys.Vec3;
import org.joml.Vector3fc;
import org.joml.Vector4fc;

/**
 * Environment attributes as a ClientLevel builds them (`EnvironmentAttributeSystem.addDefaultLayers`
 * then ClientLevel's lightning-flash layers), over vanilla's data-pack registries, for a level
 * that is only what those layers read: its dimension, clock, rain and thunder.
 */
public final class Environments {
    private static RegistryAccess.Frozen world;

    private Environments() {}

    /** The data pack's world registries with tags bound (a dimension's timelines are a tag); after Horizon's VanillaEnvironmentOracle. */
    public static synchronized RegistryAccess.Frozen world() {
        if (world != null) return world;
        var repository = ServerPacksSource.createVanillaTrustedRepository();
        repository.reload();
        repository.setSelected(List.of("vanilla"));
        var resources = new MultiPackResourceManager(PackType.SERVER_DATA, repository.openAllSelected());
        var layers = RegistryLayer.createRegistryAccess();
        var staticTags = TagLoader.loadTagsForExistingRegistries(resources, layers.getLayer(RegistryLayer.STATIC));
        var context = TagLoader.buildUpdatedLookups(layers.getAccessForLoading(RegistryLayer.WORLD), staticTags);
        world = RegistryDataLoader.load(resources, context, RegistryDataLoader.WORLD_REGISTRIES, Runnable::run).join();
        staticTags.forEach(Registry.PendingTags::apply);
        return world;
    }

    public static Holder<Biome> biome(String name) {
        return world().lookupOrThrow(Registries.BIOME).getOrThrow(ResourceKey.create(Registries.BIOME, Identifier.withDefaultNamespace(name)));
    }

    public static List<String> biomes() {
        var out = new ArrayList<String>();
        world().lookupOrThrow(Registries.BIOME).listElementIds().forEach(k -> out.add(k.identifier().getPath()));
        out.sort(null);
        return out;
    }

    /** A clock manager every clock of which stands at `ticks`. */
    public static final class Clocks extends ClientClockManager {
        long ticks;

        @Override
        public ClientClockManager.ClientClockInstance getInstance(final Holder<WorldClock> definition) {
            var clock = super.getInstance(definition);
            Headless.set(clock, "totalTicks", ticks);
            return clock;
        }
    }

    /** The level the attribute layers and the fog read; allocated bare, see Headless. */
    public static final class FakeLevel extends ClientLevel {
        Holder<DimensionType> dimensionType;
        ResourceKey<Level> dimension;
        Clocks clocks;
        float rain, thunder;
        public Holder<Biome> biome;
        // Allocated bare: no initializer runs, level() sets every field.
        public int skyLight, minY;
        public EnvironmentAttributeSystem system;
        public ClientLevel.ClientLevelData data;

        private FakeLevel() {
            super(null, null, null, null, 0, 0, (LevelExtractor) null, false, 0L, 0);
        }

        @Override public RegistryAccess registryAccess() { return world(); }
        @Override public BiomeManager getBiomeManager() {
            var biomes = Headless.allocate(CameraBiome.class);
            biomes.biome = biome;
            return biomes;
        }
        @Override public DimensionType dimensionType() { return dimensionType.value(); }
        @Override public ResourceKey<Level> dimension() { return dimension; }
        @Override public ClientClockManager clockManager() { return clocks; }
        @Override public float getRainLevel(final float a) { return rain; }
        @Override public float getThunderLevel(final float a) { return thunder; }
        @Override public EnvironmentAttributeSystem environmentAttributes() { return system; }
        @Override public ClientLevel.ClientLevelData getLevelData() { return data; }
        @Override public int getMinY() { return minY; }
        @Override public Holder<Biome> getBiome(final BlockPos pos) { return biome; }

        @Override
        public LevelLightEngine getLightEngine() {
            var sky = Headless.allocate(Light.class);
            sky.level = skyLight;
            return sky;
        }
    }

    /** `biome` everywhere: the camera's biome, for the attributes biomes set without blending. */
    public static final class CameraBiome extends BiomeManager {
        Holder<Biome> biome;

        private CameraBiome() {
            super(null, 0L);
        }

        @Override public Holder<Biome> getNoiseBiomeAtPosition(final double x, final double y, final double z) { return biome; }
        @Override public Holder<Biome> getBiome(final int x, final int y, final int z) { return biome; }
    }

    /** Sky light `level` everywhere. */
    public static final class Light extends LevelLightEngine {
        int level;

        private Light() {
            super(null, false, false);
        }

        @Override
        public LayerLightEventListener getLayerListener(final LightLayer layer) {
            return (LayerLightEventListener) java.lang.reflect.Proxy.newProxyInstance(LayerLightEventListener.class.getClassLoader(),
                new Class<?>[] {LayerLightEventListener.class},
                (proxy, method, args) -> method.getName().equals("getLightValue") ? level : Headless.zero(method.getReturnType()));
        }
    }

    /** Whether a boss bar asks for world fog. */
    public static final class Bosses extends BossHealthOverlay {
        public boolean worldFog;

        private Bosses() {
            super(null);
        }

        @Override
        public boolean shouldCreateWorldFog() {
            return worldFog;
        }
    }

    private static <T> OptionInstance<T> option(T value) {
        @SuppressWarnings("unchecked")
        OptionInstance<T> option = Headless.allocate(OptionInstance.class);
        Headless.set(option, "value", value);
        return option;
    }

    /** `Minecraft.getInstance()` with the options and boss bars the fog and sky read; returns its boss overlay. */
    public static Bosses installMinecraft(int cloudRangeChunks) {
        var minecraft = Headless.allocate(Minecraft.class);
        var options = Headless.allocate(Options.class);
        Headless.set(options, "cloudRange", option(cloudRangeChunks));
        Headless.set(options, "hideLightningFlash", option(false));
        Headless.set(minecraft, "options", options);
        var gui = Headless.allocate(Gui.class);
        var hud = Headless.allocate(Hud.class);
        var bosses = Headless.allocate(Bosses.class);
        Headless.set(hud, "bossOverlay", bosses);
        Headless.set(gui, "hud", hud);
        Headless.set(minecraft, "gui", gui);
        Headless.setStatic(Minecraft.class, "instance", minecraft);
        return bosses;
    }

    /** A level of dimension type `dimension` (also its level key) at `ticks` with the camera in `biome`; `flash` sets skyFlashTime. */
    public static FakeLevel level(String dimension, Holder<Biome> biome, long ticks, float rain, float thunder, boolean flash, boolean flat) {
        var level = Headless.allocate(FakeLevel.class);
        level.dimensionType = world().lookupOrThrow(Registries.DIMENSION_TYPE)
            .getOrThrow(ResourceKey.create(Registries.DIMENSION_TYPE, Identifier.withDefaultNamespace(dimension)));
        level.dimension = ResourceKey.create(Registries.DIMENSION, Identifier.withDefaultNamespace(dimension));
        level.clocks = new Clocks();
        level.clocks.ticks = ticks;
        level.rain = rain;
        level.thunder = thunder;
        level.skyLight = 15;
        level.biome = biome;
        level.data = Headless.allocate(ClientLevel.ClientLevelData.class);
        Headless.set(level.data, "isFlat", flat);
        if (Minecraft.getInstance() == null) installMinecraft(128);
        Headless.set(level, "minecraft", Minecraft.getInstance());
        Headless.set(level, "skyFlashTime", flash ? 2 : 0);
        // ClientLevel's own builder: the default layers, then the flash's.
        var builder = EnvironmentAttributeSystem.builder();
        Headless.call(ClientLevel.class, level, "addEnvironmentAttributeLayers", new Class<?>[] {EnvironmentAttributeSystem.Builder.class}, builder);
        level.system = builder.build();
        return level;
    }

    /** The camera's biomes and their weights, in the order the probe first met them. */
    public static SpatialAttributeInterpolator blend(List<Map.Entry<Holder<Biome>, Double>> weights) {
        var interpolator = new SpatialAttributeInterpolator();
        for (var w : weights) interpolator.accumulate(w.getValue(), w.getKey().value().getAttributes());
        return interpolator;
    }

    public static <V> V value(FakeLevel level, EnvironmentAttribute<V> attribute, SpatialAttributeInterpolator biomes) {
        return level.system.getValue(attribute, Vec3.ZERO, biomes);
    }

    /** A value as JSON: numbers, colours as float lists, enums by serialized name; null for the rest. */
    public static Object json(Object value) {
        if (value instanceof Vector3fc c) return List.of(c.x(), c.y(), c.z());
        if (value instanceof Vector4fc c) return List.of(c.x(), c.y(), c.z(), c.w());
        if (value instanceof Float || value instanceof Boolean) return value;
        if (value instanceof net.minecraft.util.StringRepresentable e) return e.getSerializedName();
        return null;
    }
}
