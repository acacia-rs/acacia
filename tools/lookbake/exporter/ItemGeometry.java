// SPDX-License-Identifier: MIT
package acacia.lookbake;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import net.minecraft.client.model.Model;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.renderer.special.SpecialModelRenderer;
import net.minecraft.client.renderer.texture.OverlayTexture;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.renderer.texture.UvMapping;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.resources.Identifier;
import org.joml.Matrix3f;
import org.joml.Matrix4fc;
import org.joml.Vector3f;

/**
 * An item layer's geometry as quads in the layer's space (its local transform applied, before the
 * display transform): per quad the image it samples (`minecraft:textures/item/apple.png`),
 * corners, texture coordinates within that image, normal, tint index (-1 for none) and ARGB colour.
 * Baked quads come straight from the model; a special model is captured from what its renderer
 * submits (models, each drawn by ModelPart.render). Submissions of any other kind are counted in
 * `skipped`.
 */
final class ItemGeometry {
    final List<JsonObject> quads = new ArrayList<>();
    final Map<String, Integer> skipped = new TreeMap<>();

    void baked(List<BakedQuad> baked, Matrix4fc local) {
        var normals = new Matrix3f().set(local).invert().transpose();
        for (BakedQuad quad : baked) {
            var info = quad.materialInfo();
            var pos = new float[12];
            var uv = new float[8];
            for (int i = 0; i < 4; i++) {
                var p = local.transformPosition(new Vector3f(quad.position(i)));
                pos[i * 3] = p.x; pos[i * 3 + 1] = p.y; pos[i * 3 + 2] = p.z;
                long packed = quad.packedUV(i);
                uv[i * 2] = Float.intBitsToFloat((int) (packed >>> 32));
                uv[i * 2 + 1] = Float.intBitsToFloat((int) packed);
            }
            var n = normals.transform(new Vector3f(quad.direction().getUnitVec3f())).normalize();
            add(image(info.sprite()), pos, uv, new float[] {n.x, n.y, n.z}, info.tintIndex(), -1);
        }
    }

    void special(SpecialModelRenderer<Object> renderer, Object argument, Matrix4fc local) {
        var stack = new PoseStack();
        stack.last().mulPose(local);
        var collector = (SubmitNodeCollector) Proxy.newProxyInstance(SubmitNodeCollector.class.getClassLoader(),
            new Class<?>[] {SubmitNodeCollector.class}, collector());
        renderer.submit(argument, stack, collector, 0xF000F0, OverlayTexture.NO_OVERLAY, false, 0);
    }

    private InvocationHandler collector() {
        return (proxy, method, args) -> {
            var name = method.getName();
            if (name.equals("order")) return proxy;
            if (name.equals("submitModel") && args.length == 9 && args[3] instanceof RenderType renderType) {
                model((Model<?>) args[0], args[1], (PoseStack) args[2], renderType, (int) args[6], (UvMapping) args[7]);
                return null;
            }
            if (method.isDefault()) return InvocationHandler.invokeDefault(proxy, method, args);
            skipped.merge(name, 1, Integer::sum);
            return null;
        };
    }

    @SuppressWarnings("unchecked")
    private void model(Model<?> model, Object state, PoseStack pose, RenderType renderType, int color, UvMapping mapping) throws Exception {
        ((Model<Object>) model).setupAnim(state);
        var capture = new Capture(mapping instanceof TextureAtlasSprite sprite ? image(sprite) : texture(renderType), color);
        model.renderToBuffer(pose, mapping == null ? capture : mapping.wrap(capture), 0xF000F0, OverlayTexture.NO_OVERLAY, color);
        capture.finish();
        model.resetPose();
    }

    private void add(String image, float[] pos, float[] uv, float[] normal, int tint, int color) {
        var quad = new JsonObject();
        quad.addProperty("image", image);
        quad.add("pos", array(pos));
        quad.add("uv", array(uv));
        quad.add("normal", array(normal));
        quad.addProperty("tint", tint);
        quad.addProperty("color", color);
        quads.add(quad);
    }

    static JsonArray array(float[] values) {
        var out = new JsonArray();
        for (float f : values) out.add(Float.isFinite(f) ? f : 0.0F);
        return out;
    }

    private static String image(TextureAtlasSprite sprite) {
        var name = sprite.contents().name();
        return name.withPath("textures/" + name.getPath() + ".png").toString();
    }

    private static Field renderState;
    private static Field textures;

    /** The texture a render type samples (its RenderSetup's Sampler0). */
    @SuppressWarnings("unchecked")
    private static String texture(RenderType type) throws Exception {
        if (renderState == null) {
            renderState = RenderType.class.getDeclaredField("state");
            renderState.setAccessible(true);
            textures = renderState.getType().getDeclaredField("textures");
            textures.setAccessible(true);
        }
        var binding = ((Map<String, ?>) textures.get(renderState.get(type))).get("Sampler0");
        var location = binding.getClass().getDeclaredMethod("location");
        location.setAccessible(true);
        return ((Identifier) location.invoke(binding)).toString();
    }

    /** Every four vertices ModelPart.render hands it make a quad. */
    private final class Capture implements VertexConsumer {
        private final String image;
        private final int color;
        private final float[] pos = new float[12], uv = new float[8], normal = new float[3];
        private int vertex = -1;

        Capture(String image, int color) {
            this.image = image;
            this.color = color;
        }

        @Override public VertexConsumer addVertex(float x, float y, float z) {
            if (vertex == 3) add(image, pos.clone(), uv.clone(), normal.clone(), -1, color);
            vertex = (vertex + 1) % 4;
            pos[vertex * 3] = x; pos[vertex * 3 + 1] = y; pos[vertex * 3 + 2] = z;
            return this;
        }
        @Override public VertexConsumer setNormal(float x, float y, float z) {
            normal[0] = x; normal[1] = y; normal[2] = z;
            return this;
        }
        @Override public VertexConsumer setUv(float u, float v) {
            uv[vertex * 2] = u; uv[vertex * 2 + 1] = v;
            return this;
        }
        @Override public VertexConsumer setColor(int r, int g, int b, int a) { return this; }
        @Override public VertexConsumer setColor(int color) { return this; }
        @Override public VertexConsumer setUv1(int u, int v) { return this; }
        @Override public VertexConsumer setUv2(int u, int v) { return this; }
        @Override public VertexConsumer setUv3(float u, float v) { return this; }
        @Override public VertexConsumer setLineWidth(float width) { return this; }

        /** The last quad, which no fifth vertex closes. */
        void finish() {
            if (vertex == 3) add(image, pos.clone(), uv.clone(), normal.clone(), -1, color);
            vertex = -1;
        }
    }
}
