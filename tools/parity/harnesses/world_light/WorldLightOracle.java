package acacia.parity.world_light;

import acacia.parity.Data;
import acacia.parity.FluidRendering;
import acacia.parity.Models;
import acacia.parity.Out;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.blaze3d.vertex.QuadInstance;
import java.io.BufferedReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.client.renderer.block.BlockAndTintGetter;
import net.minecraft.client.renderer.block.BlockModelLighter;
import net.minecraft.client.renderer.block.FluidRenderer;
import net.minecraft.client.renderer.block.dispatch.BlockStateModelPart;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.SectionPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.CardinalLighting;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.ColorResolver;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.LightLayer;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.RenderShape;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.BlockStateProperties;
import net.minecraft.world.level.chunk.LightChunk;
import net.minecraft.world.level.chunk.LightChunkGetter;
import net.minecraft.world.level.chunk.PalettedContainerFactory;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.level.chunk.UpgradeData;
import net.minecraft.world.level.lighting.LevelLightEngine;
import net.minecraft.world.level.lighting.LightEngine;
import net.minecraft.world.level.material.FluidState;

/**
 * Real Bedrock terrain (args[1]: tools/parity harness world_light's scenes, from movement
 * traces) mapped to Java states by lookbake's table (args[2]), lit by Java's LevelLightEngine and
 * drawn by ModelBlockRenderer's loop with the client's BlockModelLighter (smooth lighting on).
 * Writes quads.jsonl: per drawn quad of the scene's inner chunks, its world corners (1/256 block,
 * with the state's offset) and each vertex's light coords and grey, liquids' quads from
 * `FluidRenderer` too ({@link FluidQuads}); light.bin: per scene, the
 * inner chunks' light (block << 4 | sky), chunk by chunk (x outer), then y, z, x; and
 * unmapped.json: the Bedrock states the table has no Java state for (drawn as air).
 */
public final class WorldLightOracle {
    private static final Direction[] SIDES = {Direction.DOWN, Direction.UP, Direction.NORTH, Direction.SOUTH, Direction.WEST, Direction.EAST, null};

    /** The scene's chunks and their light, as the lighter reads a level. */
    private record Scene(Map<Long, ProtoChunk> chunks, LevelLightEngine light, boolean nether, int minY, int height) implements BlockAndTintGetter {
        public CardinalLighting cardinalLighting() { return nether ? CardinalLighting.NETHER : CardinalLighting.DEFAULT; }
        public LevelLightEngine getLightEngine() { return light; }
        public int getBrightness(LightLayer layer, BlockPos pos) { return light.getLayerListener(layer).getLightValue(pos); }
        public int getBlockTint(BlockPos pos, ColorResolver resolver) { return 0xFFFFFFFF; }
        public BlockEntity getBlockEntity(BlockPos pos) { return null; }
        public BlockState getBlockState(BlockPos pos) {
            var chunk = chunks.get(ChunkPos.pack(pos.getX() >> 4, pos.getZ() >> 4));
            return chunk == null ? Blocks.AIR.defaultBlockState() : chunk.getBlockState(pos);
        }
        public FluidState getFluidState(BlockPos pos) { return getBlockState(pos).getFluidState(); }
        public int getHeight() { return height; }
        public int getMinY() { return minY; }
    }

    public static void main(String[] args) throws Exception {
        var mapping = JsonParser.parseString(Files.readString(Path.of(args[2]))).getAsJsonObject();
        var unmapped = new java.util.TreeSet<String>();
        try (var out = new Out.Lines(Path.of(args[0], "quads.jsonl"));
             var light = new java.io.BufferedOutputStream(Files.newOutputStream(Path.of(args[0], "light.bin")));
             BufferedReader in = Files.newBufferedReader(Path.of(args[1]), StandardCharsets.UTF_8)) {
            var tools = new Tools(Models.get(), PalettedContainerFactory.create(Data.get().access), FluidRendering.renderer(), new FluidQuads(out), mapping, unmapped, out, light);
            String line;
            int index = 0;
            while ((line = in.readLine()) != null) {
                if (line.isBlank()) continue;
                scene(JsonParser.parseString(line).getAsJsonObject(), index++, tools);
            }
        }
        Files.writeString(Path.of(args[0], "unmapped.json"), new com.google.gson.Gson().toJson(unmapped));
    }

    private record Tools(Models models, PalettedContainerFactory factory, FluidRenderer fluids, FluidQuads fluidQuads, JsonObject mapping,
                         java.util.Set<String> unmapped, Out.Lines out, java.io.OutputStream light) {}

    private static BlockState parse(String state) throws Exception {
        return BlockStateParser.parseForBlock(BuiltInRegistries.BLOCK, state, false).blockState();
    }

    /** The Java state of a cell: the block layer's, waterlogged by water in the liquid layer, or the liquid's alone. */
    private static BlockState java(String block, String liquid, JsonObject mapping, java.util.Set<String> unmapped, Map<String, BlockState> cache) throws Exception {
        var b = map(block, mapping, unmapped, cache);
        if (liquid == null || liquid.equals("minecraft:air")) return b;
        if (b.isAir()) return map(liquid, mapping, unmapped, cache);
        boolean water = liquid.startsWith("minecraft:water") || liquid.startsWith("minecraft:flowing_water");
        return water && b.hasProperty(BlockStateProperties.WATERLOGGED) ? b.setValue(BlockStateProperties.WATERLOGGED, true) : b;
    }

    private static BlockState map(String key, JsonObject mapping, java.util.Set<String> unmapped, Map<String, BlockState> cache) throws Exception {
        var hit = cache.get(key);
        if (hit != null) return hit;
        var java = mapping.get(key);
        BlockState state;
        if (key.equals("minecraft:air")) state = Blocks.AIR.defaultBlockState();
        else if (java == null) {
            unmapped.add(key);
            state = Blocks.AIR.defaultBlockState();
        } else state = parse(java.getAsString());
        cache.put(key, state);
        return state;
    }

    private static void scene(JsonObject row, int index, Tools tools) throws Exception {
        String dimension = row.get("dimension").getAsString();
        int minY = row.get("min_y").getAsInt(), height = row.get("height").getAsInt();
        var level = LevelHeightAccessor.create(minY, height);
        var palette = row.getAsJsonArray("palette");
        Map<String, BlockState> cache = new HashMap<>();
        Map<Long, ProtoChunk> chunks = new HashMap<>();
        for (var c : row.getAsJsonArray("chunks")) {
            var chunkRow = c.getAsJsonObject();
            var pos = new ChunkPos(chunkRow.get("x").getAsInt(), chunkRow.get("z").getAsInt());
            var chunk = new ProtoChunk(pos, UpgradeData.EMPTY, level, tools.factory(), null);
            chunks.put(pos.pack(), chunk);
            for (var s : chunkRow.getAsJsonArray("sections")) {
                var section = s.getAsJsonObject();
                int y0 = minY + section.get("y").getAsInt() * 16;
                var blocks = section.getAsJsonArray("blocks");
                var liquid = section.get("liquid").isJsonNull() ? null : section.getAsJsonArray("liquid");
                for (int i = 0; i < 4096; i++) {
                    String b = palette.get(blocks.get(i).getAsInt()).getAsString();
                    String l = liquid == null ? null : palette.get(liquid.get(i).getAsInt()).getAsString();
                    var state = java(b, l, tools.mapping(), tools.unmapped(), cache);
                    if (!state.isAir()) chunk.setBlockState(new BlockPos(pos.getMinBlockX() + (i >> 8), y0 + (i & 15), pos.getMinBlockZ() + (i >> 4 & 15)), state, 0);
                }
            }
        }
        chunks.values().forEach(ProtoChunk::initializeLightSources);
        LightChunkGetter getter = new LightChunkGetter() {
            public LightChunk getChunkForLighting(int x, int z) { return chunks.get(ChunkPos.pack(x, z)); }
            public BlockGetter getLevel() { return EmptyBlockGetter.INSTANCE; }
        };
        boolean nether = dimension.equals("the_nether");
        var engine = new LevelLightEngine(getter, true, !nether);
        for (var chunk : chunks.values()) {
            var sections = chunk.getSections();
            for (int i = 0; i < sections.length; i++) {
                if (!sections[i].hasOnlyAir()) engine.updateSectionStatus(SectionPos.of(chunk.getPos(), level.getSectionYFromSectionIndex(i)), false);
            }
            engine.setLightEnabled(chunk.getPos(), false);
        }
        chunks.values().forEach(chunk -> engine.propagateLightSources(chunk.getPos()));
        while (engine.hasLightWork()) engine.runLightUpdates();

        var scene = new Scene(chunks, engine, nether, minY, height);
        var lighter = new BlockModelLighter();
        int[] centre = {row.getAsJsonArray("centre").get(0).getAsInt(), row.getAsJsonArray("centre").get(1).getAsInt()};
        int inner = row.get("radius").getAsInt() - 1;
        for (int cx = centre[0] - inner; cx <= centre[0] + inner; cx++) {
            for (int cz = centre[1] - inner; cz <= centre[1] + inner; cz++) {
                for (int y = minY; y < minY + height; y++) {
                    for (int z = cz * 16; z < cz * 16 + 16; z++) {
                        for (int x = cx * 16; x < cx * 16 + 16; x++) {
                            var pos = new BlockPos(x, y, z);
                            tools.light().write(scene.getBrightness(LightLayer.BLOCK, pos) << 4 | (nether ? 0 : scene.getBrightness(LightLayer.SKY, pos)));
                        }
                    }
                }
            }
        }
        BlockModelLighter.enableCaching();
        for (int cx = centre[0] - inner; cx <= centre[0] + inner; cx++) {
            for (int cz = centre[1] - inner; cz <= centre[1] + inner; cz++) {
                var chunk = chunks.get(ChunkPos.pack(cx, cz));
                for (var pos : BlockPos.betweenClosed(cx * 16, minY, cz * 16, cx * 16 + 15, minY + height - 1, cz * 16 + 15)) {
                    var state = chunk.getBlockState(pos);
                    if (!state.getFluidState().isEmpty()) {
                        tools.fluidQuads().scene = index;
                        tools.fluidQuads().pos = pos.immutable();
                        tools.fluids().tesselate(scene, pos.immutable(), layer -> tools.fluidQuads(), state, state.getFluidState());
                    }
                    if (state.getRenderShape() != RenderShape.MODEL) continue;
                    var model = tools.models().blockStateModel(state);
                    if (model == null) continue;
                    var parts = new ArrayList<BlockStateModelPart>();
                    model.collectParts(RandomSource.create(state.getSeed(pos)), parts);
                    boolean ao = !parts.isEmpty() && state.getLightEmission() == 0 && parts.getFirst().useAmbientOcclusion();
                    var offset = state.getOffset(pos);
                    for (var part : parts) {
                        for (var side : SIDES) {
                            if (side != null && !Block.shouldRenderFace(state, scene.getBlockState(pos.relative(side)), side)) continue;
                            for (var quad : part.getQuads(side)) {
                                var instance = new QuadInstance();
                                if (ao) lighter.prepareQuadAmbientOcclusion(scene, state, pos, quad, instance);
                                else {
                                    int coords = side == null ? -1 : lighter.getLightCoords(state, scene, pos.relative(side));
                                    lighter.prepareQuadFlat(scene, state, pos, coords, quad, instance);
                                }
                                tools.out().write(quad(index, pos, offset, quad, instance));
                            }
                        }
                    }
                }
            }
        }
        BlockModelLighter.clearCache();
    }

    private static Object quad(int scene, BlockPos pos, net.minecraft.world.phys.Vec3 offset, BakedQuad quad, QuadInstance instance) {
        var corners = new ArrayList<Integer>();
        var light = new ArrayList<Integer>();
        for (int i = 0; i < 4; i++) {
            var v = quad.position(i);
            corners.add((int) Math.round((pos.getX() + offset.x + v.x()) * 256.0));
            corners.add((int) Math.round((pos.getY() + offset.y + v.y()) * 256.0));
            corners.add((int) Math.round((pos.getZ() + offset.z + v.z()) * 256.0));
            int coords = instance.getLightCoords(i);
            light.add(coords & 0xFF);
            light.add(coords >> 16 & 0xFF);
            light.add(instance.getColor(i) >> 8 & 0xFF);
        }
        return Out.map("scene", scene, "pos", new int[]{pos.getX(), pos.getY(), pos.getZ()}, "corners", corners, "light", light);
    }
}
