package acacia.parity;

import java.lang.reflect.Field;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Modifier;
import java.lang.reflect.Proxy;
import java.util.*;
import net.minecraft.client.model.Model;
import net.minecraft.client.model.geom.ModelLayerLocation;
import net.minecraft.client.model.geom.ModelLayers;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.rendertype.RenderType;

/** What renderers submit, recorded: each model by the model layers whose geometry it has. */
public final class Submits {
    private Submits() {}

    /** A collector adding each model submitted to `draws`: order, layers, texture, colour, render type; other submits are dropped. */
    public static SubmitNodeCollector collector(List<Map<String, Object>> draws, Map<String, List<String>> layers) {
        InvocationHandler handler = new InvocationHandler() {
            int order;

            public Object invoke(Object proxy, java.lang.reflect.Method method, Object[] a) throws Throwable {
                if (method.getName().equals("order")) {
                    order = (Integer) a[0];
                    return proxy;
                }
                if (method.isDefault()) return InvocationHandler.invokeDefault(proxy, method, a);
                if (method.getName().equals("submitModel")) {
                    var draw = new TreeMap<String, Object>();
                    draw.put("order", order);
                    var print = print(((Model<?>) a[0]).root());
                    draw.put("layers", layers.getOrDefault(print, List.of()));
                    var renderType = (RenderType) a[3];
                    draw.put("renderType", (String) Headless.get(renderType, "name"));
                    draw.put("texture", texture(renderType));
                    draw.put("color", a[6]);
                    draws.add(draw);
                    order = 0;
                }
                return Headless.zero(method.getReturnType());
            }
        };
        return (SubmitNodeCollector) Proxy.newProxyInstance(SubmitNodeCollector.class.getClassLoader(), new Class<?>[] {SubmitNodeCollector.class}, handler);
    }

    private static String texture(RenderType renderType) {
        var textures = (Map<?, ?>) Headless.get(Headless.get(renderType, "state"), "textures");
        var binding = textures.get("Sampler0");
        return binding == null ? "" : String.valueOf(Headless.get(binding, "location"));
    }

    /** Every model layer's geometry print to its names (`minecraft:horse#saddle`), sorted. */
    public static Map<String, List<String>> layerPrints() throws Exception {
        var models = Models.get().entityModels;
        var out = new HashMap<String, List<String>>();
        for (Field f : ModelLayers.class.getDeclaredFields()) {
            if (!Modifier.isStatic(f.getModifiers()) || f.getType() != ModelLayerLocation.class) continue;
            var location = (ModelLayerLocation) f.get(null);
            try {
                var root = models.bakeLayer(location);
                var name = location.model() + "#" + location.layer();
                out.computeIfAbsent(print(root), k -> new ArrayList<>()).add(name);
                // Some models (frog, allay, vex) take the definition's `root` child as their root.
                if (root.hasChild("root")) out.computeIfAbsent(print(root.getChild("root")), k -> new ArrayList<>()).add(name);
            } catch (RuntimeException missing) {
            }
        }
        out.values().forEach(Collections::sort);
        return out;
    }

    /** The part tree's names and cube bounds, children by name. */
    @SuppressWarnings("unchecked")
    private static String print(ModelPart part) {
        var b = new StringBuilder("[");
        for (Object cube : (List<Object>) Headless.get(part, "cubes")) {
            for (String c : new String[] {"minX", "minY", "minZ", "maxX", "maxY", "maxZ"}) b.append(Headless.get(cube, c)).append(',');
            b.append(';');
        }
        var children = new TreeMap<>((Map<String, ModelPart>) Headless.get(part, "children"));
        children.forEach((name, child) -> b.append(name).append(print(child)));
        return b.append(']').toString();
    }
}
