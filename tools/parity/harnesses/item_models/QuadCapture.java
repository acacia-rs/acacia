package acacia.parity.item_models;

import com.mojang.blaze3d.vertex.VertexConsumer;
import java.util.function.BiConsumer;

/** Every four vertices ModelPart.render hands it make a quad: corners and UVs to `sink`. */
final class QuadCapture implements VertexConsumer {
    private final BiConsumer<float[], float[]> sink;
    private final float[] pos = new float[12], uv = new float[8];
    private int vertex = -1;

    QuadCapture(BiConsumer<float[], float[]> sink) {
        this.sink = sink;
    }

    @Override public VertexConsumer addVertex(float x, float y, float z) {
        if (vertex == 3) sink.accept(pos.clone(), uv.clone());
        vertex = (vertex + 1) % 4;
        pos[vertex * 3] = x; pos[vertex * 3 + 1] = y; pos[vertex * 3 + 2] = z;
        return this;
    }
    @Override public VertexConsumer setUv(float u, float v) {
        uv[vertex * 2] = u; uv[vertex * 2 + 1] = v;
        return this;
    }
    @Override public VertexConsumer setNormal(float x, float y, float z) { return this; }
    @Override public VertexConsumer setColor(int r, int g, int b, int a) { return this; }
    @Override public VertexConsumer setColor(int color) { return this; }
    @Override public VertexConsumer setUv1(int u, int v) { return this; }
    @Override public VertexConsumer setUv2(int u, int v) { return this; }
    @Override public VertexConsumer setUv3(float u, float v) { return this; }
    @Override public VertexConsumer setLineWidth(float width) { return this; }

    /** The last quad, which no fifth vertex closes. */
    void finish() {
        if (vertex == 3) sink.accept(pos.clone(), uv.clone());
        vertex = -1;
    }
}
