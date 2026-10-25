package acacia.parity;

import java.lang.reflect.Array;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.lang.reflect.RecordComponent;
import java.util.ArrayList;
import java.util.Collection;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.TreeMap;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.font.EmptyArea;
import net.minecraft.client.gui.font.TextRenderable;
import net.minecraft.client.gui.font.glyphs.BakedSheetGlyph;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.item.Item;
import net.minecraft.world.level.block.Block;
import net.minecraft.core.Holder;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.Vec3;
import org.joml.Matrix3x2fc;
import org.joml.Matrix4fc;
import org.joml.Quaternionfc;
import org.joml.Vector2fc;
import org.joml.Vector3fc;
import org.joml.Vector4fc;

/**
 * Any game value as a JSON tree for oracles to write: records by component, other objects by
 * every instance field (render states are plain field bags), joml types as float arrays, names for
 * identifiers, holders, sprites, pipelines and block states, runs for formatted text. Objects
 * outside the game's packages, cycles and anything past MAX_DEPTH become "<Type>".
 */
public final class Dump {
    private static final int MAX_DEPTH = 10;
    /** Context objects a state points back to; their contents are never the state. */
    private static final List<String> OPAQUE = List.of("net.minecraft.client.Minecraft", "net.minecraft.client.gui.Font",
        "net.minecraft.world.level.Level", "net.minecraft.world.entity.Entity", "net.minecraft.client.resources.model.ModelManager",
        "net.minecraft.client.renderer.rendertype.RenderType", "com.mojang.renderpearl.", "com.mojang.blaze3d.");

    private Dump() {}

    public static Object of(Object value) {
        return of(value, 0, new IdentityHashMap<>());
    }

    private static Object of(Object v, int depth, IdentityHashMap<Object, Boolean> path) {
        if (v == null || v instanceof Boolean || v instanceof String) return v;
        if (v instanceof Character c) return c.toString();
        if (v instanceof Float f) return f.isNaN() || f.isInfinite() ? f.toString() : f;
        if (v instanceof Double d) return d.isNaN() || d.isInfinite() ? d.toString() : d;
        if (v instanceof Number) return v;
        if (v instanceof Enum<?> e) return e.name();
        if (v instanceof Identifier || v instanceof ResourceKey<?>) return v.toString();
        if (v instanceof Holder<?> h) return h.unwrapKey().map(k -> k.identifier().toString()).orElse(h.toString());
        if (v instanceof Component c) return c.getString();
        if (v instanceof FormattedCharSequence s) return runs(s);
        if (v instanceof Font.PreparedText t) return prepared(t, depth, path);
        if (v instanceof BakedSheetGlyph g) return glyph(g);
        if (v instanceof EntityType<?> t) return EntityType.getKey(t).toString();
        if (v instanceof Item i) return BuiltInRegistries.ITEM.getKey(i).toString();
        if (v instanceof Block b) return BuiltInRegistries.BLOCK.getKey(b).toString();
        if (v instanceof Textures.NamedView n) return Textures.describe(n.id());
        if (v instanceof TextureAtlasSprite s) return s.contents().name().toString();
        if (v instanceof com.mojang.renderpearl.api.pipeline.RenderPipeline p) return p.getLocation().toString();
        if (v instanceof BlockState s) return Out.stateKey(s);
        if (v instanceof ItemStack s) return s.isEmpty() ? "empty" : s.getCount() + " " + s.typeHolder().getRegisteredName();
        if (v instanceof Vec3 p) return new double[]{p.x, p.y, p.z};
        if (v instanceof Vector2fc p) return new float[]{p.x(), p.y()};
        if (v instanceof Vector3fc p) return new float[]{p.x(), p.y(), p.z()};
        if (v instanceof Vector4fc p) return new float[]{p.x(), p.y(), p.z(), p.w()};
        if (v instanceof Quaternionfc q) return new float[]{q.x(), q.y(), q.z(), q.w()};
        if (v instanceof Matrix3x2fc m) return new float[]{m.m00(), m.m01(), m.m10(), m.m11(), m.m20(), m.m21()};
        if (v instanceof Matrix4fc m) return m.get(new float[16]);
        if (v instanceof Optional<?> o) return o.map(x -> of(x, depth, path)).orElse(null);
        if (v.getClass().isArray() && v.getClass().getComponentType().isPrimitive()) {
            var out = new ArrayList<Object>();
            for (int i = 0; i < Array.getLength(v); i++) out.add(Array.get(v, i));
            return out;
        }
        String name = v.getClass().getName();
        boolean game = name.startsWith("net.minecraft.") || name.startsWith("com.mojang.");
        if (depth >= MAX_DEPTH || path.containsKey(v) || OPAQUE.stream().anyMatch(o -> isA(v.getClass(), o))
            || (!game && !(v instanceof Collection<?>) && !(v instanceof Map<?, ?>) && !(v instanceof Object[])) || v.getClass().isSynthetic()) {
            return "<" + v.getClass().getSimpleName() + ">";
        }
        path.put(v, true);
        try {
            if (v instanceof Object[] a) return list(List.of(a), depth, path);
            if (v instanceof Collection<?> c) return list(c, depth, path);
            if (v instanceof Map<?, ?> m) {
                var out = new TreeMap<String, Object>();
                m.forEach((k, x) -> out.put(String.valueOf(of(k, depth + 1, path)), of(x, depth + 1, path)));
                return out;
            }
            var out = new TreeMap<String, Object>();
            if (v instanceof Record r) {
                for (RecordComponent c : r.getClass().getRecordComponents()) out.put(c.getName(), of(read(c, r), depth + 1, path));
                return out;
            }
            for (Class<?> k = v.getClass(); k != null && k != Object.class; k = k.getSuperclass()) {
                for (Field f : k.getDeclaredFields()) {
                    if (Modifier.isStatic(f.getModifiers()) || f.isSynthetic() || out.containsKey(f.getName())) continue;
                    f.setAccessible(true);
                    out.put(f.getName(), of(f.get(v), depth + 1, path));
                }
            }
            return out;
        } catch (ReflectiveOperationException e) {
            return "<" + e + ">";
        } finally {
            path.remove(v);
        }
    }

    /** `name` is a package prefix when it ends in '.', else an exact class name. */
    private static boolean isA(Class<?> type, String name) {
        for (Class<?> k = type; k != null; k = k.getSuperclass()) {
            if (name.endsWith(".") ? k.getName().startsWith(name) : k.getName().equals(name)) return true;
        }
        return false;
    }

    /** Laid-out text: each glyph (GlyphInstance: pen x/y, colours, style, offsets, glyph box), effect and empty area. */
    private static Object prepared(Font.PreparedText text, int depth, IdentityHashMap<Object, Boolean> path) {
        var glyphs = new ArrayList<Object>();
        var effects = new ArrayList<Object>();
        var empty = new ArrayList<Object>();
        text.visit(new Font.GlyphVisitor() {
            @Override
            public void acceptGlyph(TextRenderable.Styled glyph) {
                glyphs.add(of(glyph, depth + 1, path));
            }

            @Override
            public void acceptEffect(TextRenderable effect) {
                effects.add(Out.map("left", effect.left(), "top", effect.top(), "right", effect.right(), "bottom", effect.bottom()));
            }

            @Override
            public void acceptEmptyArea(EmptyArea area) {
                empty.add(of(area, depth + 1, path));
            }
        });
        return Out.map("bounds", of(text.bounds(), depth + 1, path), "glyphs", glyphs, "effects", effects, "empty", empty);
    }

    private static Object glyph(BakedSheetGlyph g) {
        var info = g.info();
        try {
            return Out.map("advance", info.getAdvance(), "boldOffset", info.getBoldOffset(), "shadowOffset", info.getShadowOffset(),
                "left", Shell.get(g, "left"), "right", Shell.get(g, "right"), "up", Shell.get(g, "up"), "down", Shell.get(g, "down"));
        } catch (Exception e) {
            return "<" + e + ">";
        }
    }

    private static Object read(RecordComponent c, Record r) throws ReflectiveOperationException {
        var accessor = c.getAccessor();
        accessor.setAccessible(true);
        return accessor.invoke(r);
    }

    private static List<Object> list(Collection<?> c, int depth, IdentityHashMap<Object, Boolean> path) {
        var out = new ArrayList<Object>(c.size());
        for (var x : c) out.add(of(x, depth + 1, path));
        return out;
    }

    /** Text as runs of one style: [text, rgb or null, bold, italic, underlined, strikethrough, obfuscated]. */
    public static List<Object> runs(FormattedCharSequence text) {
        var runs = new ArrayList<Object>();
        text.accept((index, style, codepoint) -> {
            Integer rgb = style.getColor() == null ? null : style.getColor().getValue();
            var flags = List.of(style.isBold(), style.isItalic(), style.isUnderlined(), style.isStrikethrough(), style.isObfuscated());
            var chars = new String(Character.toChars(codepoint));
            if (!runs.isEmpty()) {
                @SuppressWarnings("unchecked") var last = (List<Object>) runs.getLast();
                if (java.util.Objects.equals(last.get(1), rgb) && last.subList(2, 7).equals(flags)) {
                    last.set(0, last.get(0) + chars);
                    return true;
                }
            }
            var run = new ArrayList<Object>();
            run.add(chars);
            run.add(rgb);
            run.addAll(flags);
            runs.add(run);
            return true;
        });
        return runs;
    }
}
