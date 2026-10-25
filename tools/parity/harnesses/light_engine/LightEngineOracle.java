package acacia.parity.light_engine;

import acacia.parity.LightShapes;
import acacia.parity.Out;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import net.minecraft.SharedConstants;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.SectionPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagLoader;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.LightLayer;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.LightChunk;
import net.minecraft.world.level.chunk.LightChunkGetter;
import net.minecraft.world.level.chunk.PalettedContainerFactory;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.level.chunk.UpgradeData;
import net.minecraft.world.level.lighting.LevelLightEngine;
import net.minecraft.world.level.lighting.LightEngine;

/**
 * Java's own LevelLightEngine over the layouts in args[1] (JSON lines: chunks, height, sky or not,
 * blocks by state, then edits applied one by one) into layouts.jsonl: the light facts of every
 * state used, the blocks, and each cell's block and sky light after the edits. Chunks are
 * ProtoChunks lit the way ThreadedLevelLightEngine.initializeLight and lightChunk light them; an
 * edit goes through ProtoChunk.setBlockState's light path (section status, sky sources, checkBlock).
 */
public final class LightEngineOracle {
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var factory = PalettedContainerFactory.create(registries());
        try (var out = new Out.Lines(Path.of(args[0], "layouts.jsonl"))) {
            for (String line : Files.readAllLines(Path.of(args[1]), StandardCharsets.UTF_8)) {
                if (!line.isBlank()) out.write(layout(JsonParser.parseString(line).getAsJsonObject(), factory));
            }
        }
    }

    private static RegistryAccess registries() {
        var repo = ServerPacksSource.createVanillaTrustedRepository();
        repo.reload();
        repo.setSelected(List.of("vanilla"));
        var resources = new MultiPackResourceManager(PackType.SERVER_DATA, repo.openAllSelected());
        var layers = RegistryLayer.createRegistryAccess();
        var staticTags = TagLoader.loadTagsForExistingRegistries(resources, layers.getLayer(RegistryLayer.STATIC));
        var context = TagLoader.buildUpdatedLookups(layers.getAccessForLoading(RegistryLayer.WORLD), staticTags);
        RegistryAccess.Frozen world = RegistryDataLoader.load(resources, context, RegistryDataLoader.WORLD_REGISTRIES, Runnable::run).join();
        staticTags.forEach(Registry.PendingTags::apply);
        return world;
    }

    private static BlockState parse(String state) throws Exception {
        return BlockStateParser.parseForBlock(BuiltInRegistries.BLOCK, state, false).blockState();
    }

    private static Object layout(JsonObject row, PalettedContainerFactory factory) throws Exception {
        int[] chunkSpan = ints(row.getAsJsonArray("chunks"));
        int height = row.get("height").getAsInt();
        var level = LevelHeightAccessor.create(0, height);
        Map<Long, ProtoChunk> chunks = new HashMap<>();
        for (int cx = 0; cx < chunkSpan[0]; cx++) {
            for (int cz = 0; cz < chunkSpan[1]; cz++) {
                var pos = new ChunkPos(cx, cz);
                chunks.put(pos.pack(), new ProtoChunk(pos, UpgradeData.EMPTY, level, factory, null));
            }
        }
        var states = new TreeMap<String, Object>();
        for (var item : row.getAsJsonArray("blocks")) {
            var b = item.getAsJsonArray();
            var pos = new BlockPos(b.get(0).getAsInt(), b.get(1).getAsInt(), b.get(2).getAsInt());
            var state = parse(b.get(3).getAsString());
            states.put(b.get(3).getAsString(), facts(state));
            chunks.get(ChunkPos.pack(pos)).setBlockState(pos, state, 0);
        }
        chunks.values().forEach(ProtoChunk::initializeLightSources);
        LightChunkGetter getter = new LightChunkGetter() {
            public LightChunk getChunkForLighting(int x, int z) { return chunks.get(ChunkPos.pack(x, z)); }
            public BlockGetter getLevel() { return EmptyBlockGetter.INSTANCE; }
        };
        var engine = new LevelLightEngine(getter, true, row.get("sky").getAsBoolean());
        for (var chunk : chunks.values()) {
            var sections = chunk.getSections();
            for (int i = 0; i < sections.length; i++) {
                if (!sections[i].hasOnlyAir()) engine.updateSectionStatus(SectionPos.of(chunk.getPos(), level.getSectionYFromSectionIndex(i)), false);
            }
            engine.setLightEnabled(chunk.getPos(), false);
        }
        chunks.values().forEach(chunk -> engine.propagateLightSources(chunk.getPos()));
        settle(engine);
        for (var item : row.getAsJsonArray("edits")) {
            var b = item.getAsJsonArray();
            var pos = new BlockPos(b.get(0).getAsInt(), b.get(1).getAsInt(), b.get(2).getAsInt());
            var state = parse(b.get(3).getAsString());
            states.put(b.get(3).getAsString(), facts(state));
            var chunk = chunks.get(ChunkPos.pack(pos));
            var section = chunk.getSection(chunk.getSectionIndex(pos.getY()));
            boolean wasEmpty = section.hasOnlyAir();
            var old = chunk.setBlockState(pos, state, 0);
            if (section.hasOnlyAir() != wasEmpty) engine.updateSectionStatus(pos, section.hasOnlyAir());
            if (old != null && LightEngine.hasDifferentLightProperties(old, state)) {
                chunk.getSkyLightSources().update(chunk, SectionPos.sectionRelative(pos.getX()), pos.getY(), SectionPos.sectionRelative(pos.getZ()));
                engine.checkBlock(pos);
            }
            settle(engine);
        }
        var block = new StringBuilder();
        var sky = new StringBuilder();
        for (int x = 0; x < chunkSpan[0] * 16; x++) {
            for (int z = 0; z < chunkSpan[1] * 16; z++) {
                for (int y = 0; y < height; y++) {
                    var pos = new BlockPos(x, y, z);
                    block.append(Character.forDigit(engine.getLayerListener(LightLayer.BLOCK).getLightValue(pos), 16));
                    sky.append(Character.forDigit(engine.getLayerListener(LightLayer.SKY).getLightValue(pos), 16));
                }
            }
        }
        return Out.map("case", row.get("case").getAsString(), "chunks", chunkSpan, "height", height, "sky", row.get("sky").getAsBoolean(),
            "states", states, "blocks", row.get("blocks"), "edits", row.get("edits"), "block_light", block.toString(), "sky_light", sky.toString());
    }

    private static void settle(LevelLightEngine engine) {
        while (engine.hasLightWork()) engine.runLightUpdates();
    }

    /** What acacia's engine takes of a state: emission, dampening, and the face shapes it looks at. */
    private static Object facts(BlockState s) {
        var facts = Out.map("emission", s.getLightEmission(), "dampening", s.getLightDampening());
        var faces = LightShapes.of(s);
        if (faces != null) facts.put("faces", faces);
        return facts;
    }

    private static int[] ints(JsonArray a) {
        var out = new int[a.size()];
        for (int i = 0; i < out.length; i++) out[i] = a.get(i).getAsInt();
        return out;
    }
}
