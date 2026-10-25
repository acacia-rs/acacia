package acacia.parity.smooth_light;

import acacia.parity.Models;
import acacia.parity.Out;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.blaze3d.vertex.QuadInstance;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.client.renderer.block.BlockAndTintGetter;
import net.minecraft.client.renderer.block.BlockModelLighter;
import net.minecraft.client.renderer.block.dispatch.BlockStateModelPart;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.CardinalLighting;
import net.minecraft.world.level.ColorResolver;
import net.minecraft.world.level.LightLayer;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.RenderShape;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.lighting.LevelLightEngine;
import net.minecraft.world.level.material.FluidState;

/**
 * The client's own BlockModelLighter over the scenes in args[1] (JSON lines: blocks by state over
 * air, light per cell over a default, nether or not, the blocks to draw) into scenes.jsonl: every
 * quad drawn, with the facts of every cell, and its vertices' light coords and grey. The loop is
 * ModelBlockRenderer.tesselateBlock's (smooth lighting and culling on) without the tint, so the
 * grey is the lighter's alone and the cull side of each quad is known. The Rust lighter takes the
 * quad and the facts and must give back the light. Adapted from Horizon's VanillaSectionMeshOracle.
 */
public final class SmoothLightOracle {
    private static final Direction[] SIDES = {Direction.DOWN, Direction.UP, Direction.NORTH, Direction.SOUTH, Direction.WEST, Direction.EAST, null};

    /** A fixed region: listed blocks over air, listed light over a default. */
    private static final class Scene implements BlockAndTintGetter {
        final Map<Long, BlockState> blocks = new HashMap<>();
        final Map<Long, int[]> light = new HashMap<>();
        int[] defaultLight;
        boolean nether;

        public CardinalLighting cardinalLighting() { return nether ? CardinalLighting.NETHER : CardinalLighting.DEFAULT; }
        public LevelLightEngine getLightEngine() { return LevelLightEngine.EMPTY; }
        public int getBrightness(LightLayer layer, BlockPos pos) {
            var value = light.getOrDefault(pos.asLong(), defaultLight);
            return layer == LightLayer.SKY ? value[0] : value[1];
        }
        public int getBlockTint(BlockPos pos, ColorResolver resolver) { return 0xFFFFFFFF; }
        public BlockEntity getBlockEntity(BlockPos pos) { return null; }
        public BlockState getBlockState(BlockPos pos) { return blocks.getOrDefault(pos.asLong(), Blocks.AIR.defaultBlockState()); }
        public FluidState getFluidState(BlockPos pos) { return getBlockState(pos).getFluidState(); }
        public int getHeight() { return 384; }
        public int getMinY() { return -64; }
    }

    private static BlockState parse(String state) throws Exception {
        return BlockStateParser.parseForBlock(BuiltInRegistries.BLOCK, state, false).blockState();
    }

    private static int[] ints(JsonArray a) {
        var out = new int[a.size()];
        for (int i = 0; i < out.length; i++) out[i] = a.get(i).getAsInt();
        return out;
    }

    /** What the lighter reads of a cell. */
    private static Object facts(Scene scene, BlockPos pos) {
        var s = scene.getBlockState(pos);
        return Out.map("pos", new int[]{pos.getX(), pos.getY(), pos.getZ()}, "emission", s.getLightEmission(), "dampening", s.getLightDampening(), "solid_render", s.isSolidRender(),
            "full_collision", s.isCollisionShapeFullBlock(scene, pos), "emissive", s.emissiveRendering(),
            "dark", s.getShadeBrightness(scene, pos) < 1.0F);
    }

    public static void main(String[] args) throws Exception {
        var models = Models.get();
        var lighter = new BlockModelLighter();
        try (var out = new Out.Lines(Path.of(args[0], "scenes.jsonl"))) {
            for (String line : Files.readAllLines(Path.of(args[1]), StandardCharsets.UTF_8)) {
                if (line.isBlank()) continue;
                JsonObject row = JsonParser.parseString(line).getAsJsonObject();
                var scene = new Scene();
                scene.nether = row.get("nether").getAsBoolean();
                scene.defaultLight = ints(row.getAsJsonArray("light"));
                for (var item : row.getAsJsonArray("blocks")) {
                    var b = item.getAsJsonArray();
                    scene.blocks.put(BlockPos.asLong(b.get(0).getAsInt(), b.get(1).getAsInt(), b.get(2).getAsInt()), parse(b.get(3).getAsString()));
                }
                for (var item : row.getAsJsonArray("lights")) {
                    var l = ints(item.getAsJsonArray());
                    scene.light.put(BlockPos.asLong(l[0], l[1], l[2]), new int[]{l[3], l[4]});
                }
                var quads = new ArrayList<Object>();
                var cells = new ArrayList<Object>();
                int[] size = ints(row.getAsJsonArray("size"));
                for (var pos : BlockPos.betweenClosed(0, 0, 0, size[0] - 1, size[1] - 1, size[2] - 1)) {
                    cells.add(facts(scene, pos));
                }
                BlockModelLighter.enableCaching();
                for (var item : row.getAsJsonArray("draw")) {
                    var p = ints(item.getAsJsonArray());
                    var pos = new BlockPos(p[0], p[1], p[2]);
                    var state = scene.getBlockState(pos);
                    var model = models.blockStateModel(state);
                    if (state.getRenderShape() != RenderShape.MODEL || model == null) continue;
                    var parts = new ArrayList<BlockStateModelPart>();
                    model.collectParts(RandomSource.create(state.getSeed(pos)), parts);
                    boolean ao = !parts.isEmpty() && state.getLightEmission() == 0 && parts.getFirst().useAmbientOcclusion();
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
                                quads.add(quad(p, quad, instance, ao, side));
                            }
                        }
                    }
                }
                BlockModelLighter.clearCache();
                out.write(Out.map("case", row.get("case").getAsString(), "nether", scene.nether, "size", size, "cells", cells,
                    "light", row.get("light"), "lights", row.get("lights"), "quads", quads));
            }
        }
    }

    private static Object quad(int[] pos, BakedQuad quad, QuadInstance instance, boolean ao, Direction cull) {
        var positions = new ArrayList<List<Float>>();
        var coords = new ArrayList<Integer>();
        var grey = new ArrayList<Integer>();
        for (int i = 0; i < 4; i++) {
            var v = quad.position(i);
            positions.add(List.of(v.x(), v.y(), v.z()));
            coords.add(instance.getLightCoords(i));
            grey.add(instance.getColor(i) >> 8 & 0xFF);
        }
        var info = quad.materialInfo();
        var override = info.shadeDirectionOverride();
        return Out.map("pos", pos, "positions", positions, "direction", quad.direction().ordinal(),
            "shade", (override != null ? override : quad.direction()).ordinal(),
            "cull", cull == null ? null : cull.ordinal(), "ao", ao, "coords", coords, "grey", grey);
    }
}
