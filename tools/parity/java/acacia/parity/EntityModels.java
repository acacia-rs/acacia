package acacia.parity;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import com.mojang.math.Axis;
import java.lang.reflect.Field;
import java.util.*;
import net.minecraft.client.model.geom.LayerDefinitions;
import net.minecraft.client.model.geom.ModelLayerLocation;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.model.geom.builders.LayerDefinition;
import net.minecraft.client.renderer.texture.OverlayTexture;

/**
 * The client's entity model layers and ModelPart.render's walk, for the entity harnesses
 * (entity_models, entity_poses). Adapted from Horizon's VanillaEntityAnimOracle (MIT).
 */
public final class EntityModels {
    private static Field cubes;
    private static Field children;
    private static Map<String, Map.Entry<ModelLayerLocation, LayerDefinition>> layers;

    private EntityModels() {}

    /** Every layer by name (`minecraft:cow#main`), in name order. */
    public static synchronized Map<String, Map.Entry<ModelLayerLocation, LayerDefinition>> layers() throws Exception {
        if (layers == null) {
            cubes = ModelPart.class.getDeclaredField("cubes");
            cubes.setAccessible(true);
            children = ModelPart.class.getDeclaredField("children");
            children.setAccessible(true);
            layers = new TreeMap<>();
            for (var entry : LayerDefinitions.createRoots().entrySet()) layers.put(entry.getKey().toString(), entry);
        }
        return layers;
    }

    @SuppressWarnings("unchecked")
    public static List<ModelPart.Cube> cubes(ModelPart part) throws Exception {
        return (List<ModelPart.Cube>) cubes.get(part);
    }

    @SuppressWarnings("unchecked")
    public static Map<String, ModelPart> children(ModelPart part) throws Exception {
        return (Map<String, ModelPart>) children.get(part);
    }

    /** Per part path (`body/head`, the root's ""): x, y, z, xRot, yRot, zRot, xScale, yScale, zScale, visible, skipDraw. */
    public static void poses(String path, ModelPart part, Map<String, Object> out) throws Exception {
        out.put(path, List.of(part.x, part.y, part.z, part.xRot, part.yRot, part.zRot, part.xScale, part.yScale, part.zScale,
            part.visible, part.skipDraw));
        for (var entry : children(part).entrySet()) poses(join(path, entry.getKey()), entry.getValue(), out);
    }

    /** LivingEntityRenderer's root transform without the entity's position. */
    public static PoseStack livingRoot(float yaw, float[] scale) {
        var stack = new PoseStack();
        stack.rotateDegrees(Axis.YP, 180.0F - yaw);
        stack.scale(-1.0F, -1.0F, 1.0F);
        stack.scale(scale[0], scale[1], scale[2]);
        stack.translate(0.0F, -1.501F, 0.0F);
        return stack;
    }

    /** ModelPart.render, each drawn part's corners kept under its path as [x, y, z, nx, ny, nz, u, v]. */
    public static void draw(String path, ModelPart part, PoseStack stack, Map<String, List<float[]>> out) throws Exception {
        if (!part.visible) return;
        var cubeList = cubes(part);
        var kids = children(part);
        if (cubeList.isEmpty() && kids.isEmpty()) return;
        stack.pushPose();
        part.translateAndRotate(stack);
        if (!part.skipDraw) {
            var capture = new Capture();
            for (ModelPart.Cube cube : cubeList) cube.compile(stack.last(), capture, 0xF000F0, OverlayTexture.NO_OVERLAY, -1);
            if (!capture.corners.isEmpty()) out.put(path, capture.corners);
        }
        for (var entry : kids.entrySet()) draw(join(path, entry.getKey()), entry.getValue(), stack, out);
        stack.popPose();
    }

    private static String join(String path, String name) {
        return path.isEmpty() ? name : path + "/" + name;
    }

    /** What Cube.compile hands its vertex consumer. A zero-size face's NaNs read 0. */
    private static final class Capture implements VertexConsumer {
        final List<float[]> corners = new ArrayList<>();
        private float[] current;

        @Override public VertexConsumer addVertex(float x, float y, float z) {
            current = new float[] {x, y, z, 0, 0, 0, 0, 0};
            corners.add(current);
            return this;
        }
        @Override public VertexConsumer setNormal(float x, float y, float z) {
            current[3] = finite(x); current[4] = finite(y); current[5] = finite(z);
            return this;
        }
        @Override public VertexConsumer setUv(float u, float v) {
            current[6] = finite(u); current[7] = finite(v);
            return this;
        }
        private static float finite(float f) {
            return Float.isFinite(f) ? f : 0.0F;
        }
        @Override public VertexConsumer setColor(int r, int g, int b, int a) { return this; }
        @Override public VertexConsumer setColor(int color) { return this; }
        @Override public VertexConsumer setUv1(int u, int v) { return this; }
        @Override public VertexConsumer setUv2(int u, int v) { return this; }
        @Override public VertexConsumer setUv3(float u, float v) { return this; }
        @Override public VertexConsumer setLineWidth(float width) { return this; }
    }
}
