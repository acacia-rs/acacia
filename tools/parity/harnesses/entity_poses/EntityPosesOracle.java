package acacia.parity.entity_poses;

import acacia.parity.EntityModels;
import acacia.parity.Game;
import acacia.parity.Out;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.lang.reflect.Field;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.lang.reflect.TypeVariable;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.*;
import net.minecraft.client.model.Model;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.core.component.DataComponentMap;
import net.minecraft.core.component.PatchedDataComponentMap;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.component.SwingAnimation;

/**
 * Model.setupAnim on render states (docs/parity.md, harness entity_poses). args[1] is a JSON-lines
 * file of cases {case, layer, model (class name), state (field: value; a leading `?` marks one the
 * state class may lack), swingHand}; each case's record in args[0]/poses.jsonl repeats them and
 * adds the posed parts and drawn corners, as EntityModelsOracle's. After Horizon's
 * VanillaEntityAnimOracle (MIT).
 */
public final class EntityPosesOracle {
    public static void main(String[] args) throws Exception {
        Game.init();
        var layers = EntityModels.layers();
        try (var out = new Out.Lines(Path.of(args[0]).resolve("poses.jsonl"))) {
            for (String line : Files.readAllLines(Path.of(args[1]))) {
                if (line.isBlank()) continue;
                var kase = JsonParser.parseString(line).getAsJsonObject();
                var record = new TreeMap<String, Object>();
                for (var e : kase.entrySet()) record.put(e.getKey(), e.getValue());
                try {
                    var layer = layers.get(kase.get("layer").getAsString());
                    if (layer == null) throw new IllegalArgumentException("no layer");
                    var root = layer.getValue().bakeRoot();
                    var cls = Class.forName(kase.get("model").getAsString());
                    @SuppressWarnings("unchecked")
                    var model = (Model<EntityRenderState>) cls.getConstructor(ModelPart.class).newInstance(root);
                    var state = (EntityRenderState) stateClass(cls).getDeclaredConstructor().newInstance();
                    for (var e : kase.getAsJsonObject("state").entrySet()) set(state, e.getKey(), e.getValue());
                    if (kase.has("swingHand") && state instanceof net.minecraft.client.renderer.entity.state.ArmedEntityRenderState armed)
                        armed.currentSwing = new LivingEntity.SwingDescription(InteractionHand.valueOf(kase.get("swingHand").getAsString()), SwingAnimation.DEFAULT, 6);
                    model.setupAnim(state);
                    var parts = new TreeMap<String, Object>();
                    EntityModels.poses("", root, parts);
                    var drawn = new TreeMap<String, List<float[]>>();
                    EntityModels.draw("", root, EntityModels.livingRoot(0.0F, new float[] {1, 1, 1}), drawn);
                    record.put("parts", parts);
                    record.put("drawn", drawn);
                } catch (Throwable error) {
                    var cause = error.getCause() != null ? error.getCause() : error;
                    record.put("error", cause.toString());
                }
                out.write(record);
            }
        }
    }

    private static Class<?> raw(Type t, Map<TypeVariable<?>, Type> env) {
        if (t instanceof Class<?> c) return c;
        if (t instanceof ParameterizedType p) return raw(p.getRawType(), env);
        if (t instanceof TypeVariable<?> v) {
            var bound = env.get(v);
            return raw(bound != null ? bound : v.getBounds()[0], env);
        }
        throw new IllegalArgumentException("cannot resolve " + t);
    }

    /** The model's render state class: Model's `S`, resolved through the generic superclasses. */
    private static Class<?> stateClass(Class<?> model) {
        var env = new HashMap<TypeVariable<?>, Type>();
        Class<?> current = model;
        for (var v : current.getTypeParameters()) env.put(v, raw(v.getBounds()[0], env));
        while (current != Model.class) {
            var sup = current.getGenericSuperclass();
            if (sup instanceof ParameterizedType p) {
                var params = ((Class<?>) p.getRawType()).getTypeParameters();
                for (int i = 0; i < params.length; i++) env.put(params[i], raw(p.getActualTypeArguments()[i], env));
                current = (Class<?>) p.getRawType();
            } else {
                current = (Class<?>) sup;
            }
        }
        return raw(Model.class.getTypeParameters()[0], env);
    }

    private static Field field(Class<?> type, String name) {
        for (Class<?> c = type; c != null; c = c.getSuperclass()) {
            try {
                return c.getDeclaredField(name);
            } catch (NoSuchFieldException e) {
            }
        }
        return null;
    }

    private static void set(Object state, String name, JsonElement value) throws Exception {
        boolean optional = name.startsWith("?");
        if (optional) name = name.substring(1);
        Field f = field(state.getClass(), name);
        if (f == null && optional) return;
        if (f == null) throw new IllegalArgumentException("no field " + name + " on " + state.getClass());
        f.setAccessible(true);
        Class<?> t = f.getType();
        if (t == float.class) f.setFloat(state, value.getAsFloat());
        else if (t == int.class) f.setInt(state, value.getAsInt());
        else if (t == boolean.class) f.setBoolean(state, value.getAsBoolean());
        else if (t.isEnum()) {
            for (Object c : t.getEnumConstants()) if (((Enum<?>) c).name().equals(value.getAsString())) f.set(state, c);
        } else if (t == ItemStack.class) {
            // Item components are not bound outside a level; models only test the stack is not empty.
            var item = BuiltInRegistries.ITEM.wrapAsHolder(BuiltInRegistries.ITEM.getValue(Identifier.parse(value.getAsString())));
            var ctor = ItemStack.class.getDeclaredConstructor(net.minecraft.core.Holder.class, int.class, PatchedDataComponentMap.class);
            ctor.setAccessible(true);
            f.set(state, ctor.newInstance(item, 1, new PatchedDataComponentMap(DataComponentMap.EMPTY)));
        } else throw new IllegalArgumentException("unsupported type " + t + " for " + name);
    }
}
