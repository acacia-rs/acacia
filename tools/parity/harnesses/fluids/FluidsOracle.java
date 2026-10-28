package acacia.parity.fluids;

import acacia.parity.FluidRendering;
import acacia.parity.Out;
import com.mojang.blaze3d.vertex.VertexConsumer;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import net.minecraft.client.renderer.block.FluidRenderer;
import net.minecraft.client.renderer.chunk.ChunkSectionLayer;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.block.LiquidBlock;

/**
 * FluidRenderer.tesselate ({@link FluidRendering}) over every liquid cell of each case in
 * {@link Cases}. Writes `cases.jsonl` (the blocks, as Java state keys) and `quads.jsonl` (the four
 * vertices of each quad the renderer emits: section-local position, sprite-local UV, ARGB colour,
 * packed light).
 */
public final class FluidsOracle {
    private FluidsOracle() {}

    private static final class Recorder implements VertexConsumer {
        final List<Object> quads = new ArrayList<>();
        List<Object> pending = new ArrayList<>();
        int[] block;
        String layer;

        @Override
        public void addVertex(float x, float y, float z, int color, float u, float v, int overlay, int light, float nx, float ny, float nz) {
            int slot = (int) Math.floor(u * FluidRendering.STRIP / 2.0f);
            pending.add(new Object[]{x, y, z, u * FluidRendering.STRIP - 2 * slot, v, color & 0xFFFFFF, color >>> 24, light, FluidRendering.SPRITES[slot]});
            if (pending.size() == 4) {
                quads.add(Out.map("block", block, "layer", layer, "vertices", pending));
                pending = new ArrayList<>();
            }
        }

        public VertexConsumer addVertex(float x, float y, float z) { throw new AssertionError(); }
        public VertexConsumer setColor(int r, int g, int b, int a) { throw new AssertionError(); }
        public VertexConsumer setColor(int color) { throw new AssertionError(); }
        public VertexConsumer setUv(float u, float v) { throw new AssertionError(); }
        public VertexConsumer setUv1(int u, int v) { throw new AssertionError(); }
        public VertexConsumer setUv2(int u, int v) { throw new AssertionError(); }
        public VertexConsumer setUv3(float u, float v) { throw new AssertionError(); }
        public VertexConsumer setNormal(float x, float y, float z) { throw new AssertionError(); }
        public VertexConsumer setLineWidth(float width) { throw new AssertionError(); }
    }

    private static List<Object> tesselate(Scene scene, FluidRenderer renderer) {
        var recorder = new Recorder();
        for (var pos : BlockPos.betweenClosed(0, Scene.MIN_Y, 0, 15, Scene.MIN_Y + 15, 15)) {
            var state = scene.getBlockState(pos);
            var fluid = state.getFluidState();
            if (fluid.isEmpty()) continue;
            recorder.block = new int[]{pos.getX(), pos.getY(), pos.getZ()};
            renderer.tesselate(scene, pos.immutable(), (ChunkSectionLayer layer) -> {
                recorder.layer = layer.name().toLowerCase(java.util.Locale.ROOT);
                return recorder;
            }, state, fluid);
        }
        if (!recorder.pending.isEmpty()) throw new AssertionError("partial quad");
        return recorder.quads;
    }

    /** `[x, y, z, state, liquid]`, `liquid` the state of a waterlogged block's water (null for none or a liquid block). */
    private static List<Object> blocks(Scene scene) {
        var out = new ArrayList<Object>();
        scene.blocks.forEach((key, state) -> {
            var p = BlockPos.of(key);
            var fluid = state.getFluidState();
            var liquid = fluid.isEmpty() || state.getBlock() instanceof LiquidBlock ? null : Out.stateKey(fluid.createLegacyBlock());
            out.add(Arrays.asList(p.getX(), p.getY(), p.getZ(), Out.stateKey(state), liquid));
        });
        return out;
    }

    public static void main(String[] args) throws Exception {
        var out = Path.of(args[0]);
        var renderer = FluidRendering.renderer();
        int count = 0;
        try (var cases = new Out.Lines(out.resolve("cases.jsonl")); var quads = new Out.Lines(out.resolve("quads.jsonl"))) {
            for (var scene : Cases.all()) {
                cases.write(Out.map("case", scene.name, "biome", scene.biomeName, "nether", scene.nether, "blocks", blocks(scene)));
                quads.write(Out.map("case", scene.name, "quads", tesselate(scene, renderer)));
                count++;
            }
        }
        Facts.write(out.resolve("facts.jsonl"));
        System.out.println("fluids: " + count + " cases");
    }
}
