package acacia.parity.world_light;

import acacia.parity.Out;
import com.mojang.blaze3d.vertex.VertexConsumer;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.BlockPos;

/**
 * Takes `FluidRenderer`'s vertices (section-local) as quads.jsonl rows like the block quads, with
 * `liquid` set: world corners in 1/256 block, and per vertex block light, sky light and the red
 * channel of its colour (the shade, the level's tint being white).
 */
final class FluidQuads implements VertexConsumer {
    private final Out.Lines out;
    private final List<Integer> corners = new ArrayList<>();
    private final List<Integer> light = new ArrayList<>();
    int scene;
    BlockPos pos;

    FluidQuads(Out.Lines out) {
        this.out = out;
    }

    @Override
    public void addVertex(float x, float y, float z, int color, float u, float v, int overlay, int coords, float nx, float ny, float nz) {
        corners.add(Math.round(((pos.getX() & ~15) + x) * 256.0f));
        corners.add(Math.round(((pos.getY() & ~15) + y) * 256.0f));
        corners.add(Math.round(((pos.getZ() & ~15) + z) * 256.0f));
        light.add(coords & 0xFF);
        light.add(coords >> 16 & 0xFF);
        light.add(color >> 16 & 0xFF);
        if (corners.size() == 12) {
            try {
                out.write(Out.map("scene", scene, "pos", new int[]{pos.getX(), pos.getY(), pos.getZ()}, "liquid", true, "corners", List.copyOf(corners), "light", List.copyOf(light)));
            } catch (java.io.IOException e) {
                throw new java.io.UncheckedIOException(e);
            }
            corners.clear();
            light.clear();
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
