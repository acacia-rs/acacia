package acacia.parity;

import com.mojang.blaze3d.platform.Window;
import java.nio.file.Files;
import java.util.Map;
import net.minecraft.client.ClientClockManager;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.renderer.block.BlockModelResolver;
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
import net.minecraft.client.renderer.extract.LevelExtractor;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.EntityType;
import net.minecraft.client.renderer.item.ItemModelResolver;
import net.minecraft.client.resources.model.EquipmentAssetManager;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.protocol.game.ClientboundLevelChunkPacketData;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.server.packs.resources.SimpleJsonResourceReloadListener;
import net.minecraft.util.datafix.DataFixers;
import net.minecraft.util.profiling.InactiveProfiler;
import net.minecraft.util.profiling.ProfilerFiller;
import net.minecraft.world.Difficulty;
import net.minecraft.world.flag.FeatureFlags;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.dimension.BuiltinDimensionTypes;
import net.minecraft.world.level.dimension.DimensionType;

/**
 * A headless client: a Minecraft shell installed as Minecraft.getInstance() holding real Options,
 * a Window shell of a chosen size and GUI scale, Fonts, Atlases, Models, an EntityRenderDispatcher
 * reloaded over the client resources, and real ClientLevels on a stand-in connection that carries
 * the data-pack registries (Data). What a code path reads that is not here stays null: set it on
 * `Client.minecraft()` with Shell.set before running it.
 */
public final class Client {
    private static Minecraft minecraft;
    private static EntityRenderDispatcher entities;

    private Client() {}

    /** Takes the level's dirty-section calls and drops them: nothing is meshed. Allocated, never constructed. */
    static final class NoExtractor extends LevelExtractor {
        private NoExtractor() {
            super(null, null, null);
        }

        @Override public void blockChanged(BlockPos pos, int flags) {}
        @Override public void setBlockDirty(BlockPos pos, BlockState oldState, BlockState newState) {}
        @Override public void setSectionDirtyWithNeighbors(int x, int y, int z) {}
        @Override public void setSectionRangeDirty(int x0, int y0, int z0, int x1, int y1, int z1) {}
    }

    private static int nextEntityId = 1;

    static synchronized int nextEntityId() {
        return nextEntityId++;
    }

    /** A new entity of `type` at (x, y, z) with an id, added to `level` as a spawn packet would add it. */
    public static <T extends Entity> T spawn(ClientLevel level, EntityType<T> type, double x, double y, double z) {
        T entity = type.create(level, EntitySpawnReason.LOAD);
        entity.setId(nextEntityId());
        entity.snapTo(x, y, z, 0.0F, 0.0F);
        level.addEntity(entity);
        return entity;
    }

    public static synchronized Minecraft minecraft() throws Exception {
        if (minecraft != null) return minecraft;
        Game.init();
        var mc = Shell.alloc(Minecraft.class);
        Shell.set(Minecraft.class, "instance", mc);
        Shell.set(mc, "fixerUpper", DataFixers.getDataFixer());
        var gameDirectory = Files.createTempDirectory("parity-client").toFile();
        gameDirectory.deleteOnExit();
        Shell.set(mc, "gameDirectory", gameDirectory);
        Shell.set(mc, "options", new Options(mc, gameDirectory));
        Shell.set(mc, "deltaTracker", new DeltaTracker.Timer(20.0F, 0L, f -> f));
        Shell.set(mc, "window", Shell.alloc(Window.class));
        Shell.set(mc, "font", Fonts.font());
        Shell.set(mc, "blockColors", Game.blockColors());
        Shell.set(mc, "atlasManager", Atlases.manager());
        Shell.set(mc, "textureManager", Textures.manager(Atlases.byTexture()));
        var models = Models.get();
        Shell.set(mc, "modelManager", models.manager);
        Shell.set(mc, "itemModelResolver", new ItemModelResolver(models.manager));
        minecraft = mc;
        window(854, 480, 2);
        var gui = Shell.alloc(net.minecraft.client.gui.Gui.class);
        Shell.set(gui, "minecraft", mc);
        Shell.set(gui, "hud", new net.minecraft.client.gui.Hud(mc));
        Shell.set(mc, "gui", gui);
        return mc;
    }

    /** The window in pixels and the GUI scale (Window.setGuiScale's rounding: scaled = ceil(pixels / scale)). */
    public static void window(int width, int height, int guiScale) throws Exception {
        var window = minecraft().getWindow();
        for (var name : new String[]{"width", "windowedWidth", "framebufferWidth"}) Shell.set(window, name, width);
        for (var name : new String[]{"height", "windowedHeight", "framebufferHeight"}) Shell.set(window, name, height);
        Shell.set(window, "guiScale", guiScale);
        Shell.set(window, "guiScaledWidth", (width + guiScale - 1) / guiScale);
        Shell.set(window, "guiScaledHeight", (height + guiScale - 1) / guiScale);
    }

    public static Options options() throws Exception {
        return minecraft().options;
    }

    /**
     * A fresh real ClientLevel of `dimensionType` (BuiltinDimensionTypes.OVERWORLD...) on its own
     * stand-in connection (registries, features, clocks), set as Minecraft's level.
     */
    public static ClientLevel level(ResourceKey<Level> dimension, ResourceKey<DimensionType> dimensionType) throws Exception {
        var mc = minecraft();
        var data = Data.get();
        var connection = Shell.alloc(ClientPacketListener.class);
        Shell.set(connection, "registryAccess", data.access);
        Shell.set(connection, "enabledFeatures", FeatureFlags.DEFAULT_FLAGS);
        Shell.set(connection, "minecraft", mc);
        Shell.set(connection, "clockManager", new ClientClockManager());
        Shell.set(connection, "playerInfoMap", new java.util.HashMap<>());
        Shell.set(connection, "listedPlayers", new java.util.HashSet<>());
        var type = data.access.lookupOrThrow(Registries.DIMENSION_TYPE).getOrThrow(dimensionType);
        var info = new ClientLevel.ClientLevelData(Difficulty.NORMAL, false, false);
        var level = new ClientLevel(connection, info, dimension, type, 8, 8, Shell.alloc(NoExtractor.class), false, 0L, 63);
        Shell.set(connection, "level", level);
        Shell.set(mc, "level", level);
        return level;
    }

    public static ClientLevel overworld() throws Exception {
        return level(Level.OVERWORLD, BuiltinDimensionTypes.OVERWORLD);
    }

    /** Chunk (cx, cz), loaded empty (air, plains) through a chunk packet if it is not loaded yet. */
    public static LevelChunk chunk(ClientLevel level, int cx, int cz) {
        if (!level.getChunkSource().hasChunk(cx, cz)) {
            var empty = new LevelChunk(level, new ChunkPos(cx, cz));
            level.getChunkSource().replaceWithPacketData(cx, cz, new ClientboundLevelChunkPacketData(empty));
        }
        return level.getChunk(cx, cz);
    }

    /** Sets a block, loading its chunk first. No neighbour updates. */
    public static void setBlock(ClientLevel level, BlockPos pos, BlockState state) {
        chunk(level, pos.getX() >> 4, pos.getZ() >> 4);
        level.setBlock(pos, state, 2 | 16);
    }

    /** Fills chunk (cx, cz) with one biome over its full height. */
    public static void setBiome(ClientLevel level, int cx, int cz, ResourceKey<Biome> biome) {
        var holder = level.registryAccess().lookupOrThrow(Registries.BIOME).getOrThrow(biome);
        chunk(level, cx, cz).fillBiomesFromNoise((x, y, z) -> holder);
    }

    /** Every entity renderer, created as EntityRenderDispatcher's reload creates them. */
    public static synchronized EntityRenderDispatcher entityRenderers() throws Exception {
        if (entities != null) return entities;
        var mc = minecraft();
        var models = Models.get();
        var equipment = new EquipmentAssetManager();
        ResourceManager resources = Game.resources();
        ProfilerFiller profiler = InactiveProfiler.INSTANCE;
        Map<?, ?> prepared = Shell.call(SimpleJsonResourceReloadListener.class, equipment, "prepare",
            new Class<?>[]{ResourceManager.class, ProfilerFiller.class}, resources, profiler);
        Shell.call(EquipmentAssetManager.class, equipment, "apply", new Class<?>[]{Map.class, ResourceManager.class, ProfilerFiller.class},
            prepared, resources, profiler);
        entities = new EntityRenderDispatcher(mc.getTextureManager(),new BlockModelResolver(models.manager), mc.getItemModelResolver(), null,
            Atlases.manager(), mc.font, mc.options, () -> models.entityModels, equipment, null, null);
        entities.onResourceManagerReload(resources);
        Shell.set(mc, "entityRenderDispatcher", entities);
        return entities;
    }
}
