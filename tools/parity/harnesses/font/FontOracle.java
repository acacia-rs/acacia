package acacia.parity.gui;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.blaze3d.vertex.VertexConsumer;
import com.mojang.serialization.JsonOps;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.EnumSet;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.font.FontOption;
import net.minecraft.client.gui.font.FontSet;
import net.minecraft.client.gui.font.TextRenderable;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import net.minecraft.client.gui.font.glyphs.BakedSheetGlyph;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.ComponentSerialization;
import net.minecraft.network.chat.FontDescription;
import net.minecraft.resources.Identifier;
import org.joml.Matrix4f;

/**
 * The `font` harness's oracle: vanilla's FontManager, FontSet and Font (GuiFonts) on the cases of
 * cases.py. `text`: Font.width of the component and the quads Font.prepareText's glyphs and
 * effects render (GlyphRenderState's flat GUI render, at the identity pose), with the builder's
 * bounds; `glyphs`: every glyph of a font in a codepoint range that is not the missing glyph;
 * `bywidth`: FontSet.glyphsByWidth in its order. Arguments: the out dir and the cases file.
 */
public final class FontOracle {
    private static final Map<String, GuiFonts> FONTS = new HashMap<>();

    static GuiFonts fonts(JsonObject c) throws Exception {
        boolean uniform = flag(c, "uniform");
        boolean jp = flag(c, "jp");
        String key = uniform + "," + jp;
        if (!FONTS.containsKey(key)) {
            var options = EnumSet.noneOf(FontOption.class);
            if (uniform) options.add(FontOption.UNIFORM);
            if (jp) options.add(FontOption.JAPANESE_VARIANTS);
            FONTS.put(key, key.equals("false,false") ? GuiGame.fonts : new GuiFonts(GuiGame.resources, options));
        }
        return FONTS.get(key);
    }

    static boolean flag(JsonObject c, String k) {
        return c.has(k) && c.get(k).getAsBoolean();
    }

    /** Vertices as GlyphRenderState hands them to the buffer: [x, y, u, v] per vertex, colour and depth per quad. */
    static final class Capture implements VertexConsumer {
        final List<float[]> vertices = new ArrayList<>();
        final List<Integer> colors = new ArrayList<>();
        float[] last;

        @Override public VertexConsumer addVertex(float x, float y, float z) {
            last = new float[] {x, y, 0, 0, z};
            vertices.add(last);
            return this;
        }
        @Override public VertexConsumer setColor(int r, int g, int b, int a) { colors.add(a << 24 | r << 16 | g << 8 | b); return this; }
        @Override public VertexConsumer setColor(int color) { colors.add(color); return this; }
        @Override public VertexConsumer setUv(float u, float v) { last[2] = u; last[3] = v; return this; }
        @Override public VertexConsumer setUv1(int u, int v) { return this; }
        @Override public VertexConsumer setUv2(int u, int v) { return this; }
        @Override public VertexConsumer setUv3(float u, float v) { return this; }
        @Override public VertexConsumer setNormal(float x, float y, float z) { return this; }
        @Override public VertexConsumer setLineWidth(float width) { return this; }
    }

    static JsonArray quads(GuiFonts fonts, Font.PreparedText prepared) {
        var out = new JsonArray();
        prepared.visit(new Font.GlyphVisitor() {
            @Override public void acceptRenderable(TextRenderable r) {
                try {
                    var glyph = (BakedSheetGlyph) Stub.get(r, r.getClass(), "glyph");
                    var capture = new Capture();
                    r.render(new Matrix4f(), capture, 15728880, true);
                    for (int q = 0; q < capture.vertices.size(); q += 4) {
                        var d = new JsonObject();
                        d.addProperty("type", "glyph");
                        d.addProperty("tex", fonts.texture(glyph));
                        d.addProperty("colored", fonts.colored(glyph));
                        d.addProperty("effect", !(r instanceof TextRenderable.Styled));
                        d.addProperty("color", capture.colors.get(q));
                        d.addProperty("z", capture.vertices.get(q)[4]);
                        var v = new JsonArray();
                        for (int i = q; i < q + 4; i++) {
                            for (int k = 0; k < 4; k++) v.add(capture.vertices.get(i)[k]);
                        }
                        d.add("v", v);
                        out.add(d);
                    }
                } catch (Exception e) {
                    throw new IllegalStateException(e);
                }
            }
        });
        return out;
    }

    static void text(JsonObject c, JsonObject result) throws Exception {
        var fonts = fonts(c);
        Font font = fonts.font(flag(c, "fishy"), c.get("seed").getAsLong());
        Component text = ComponentSerialization.CODEC.parse(GuiGame.lookup.createSerializationContext(JsonOps.INSTANCE), c.get("text")).getOrThrow();
        result.addProperty("width", font.width(text));
        int bg = c.has("bg") ? c.get("bg").getAsInt() : 0;
        var prepared = font.prepareText(text.getVisualOrderText(), c.get("x").getAsFloat(), c.get("y").getAsFloat(),
            c.get("color").getAsInt(), c.get("shadow").getAsBoolean(), false, bg);
        result.add("draws", quads(fonts, prepared));
        float l = (float) Stub.get(prepared, prepared.getClass(), "left");
        float t = (float) Stub.get(prepared, prepared.getClass(), "top");
        float r = (float) Stub.get(prepared, prepared.getClass(), "right");
        float b = (float) Stub.get(prepared, prepared.getClass(), "bottom");
        if (l < r && t < b) {
            var bounds = new JsonArray();
            for (float f : new float[] {l, t, r, b}) bounds.add(f);
            result.add("bounds", bounds);
        } else {
            result.add("bounds", JsonNull.INSTANCE);
        }
    }

    static FontSet set(JsonObject c) throws Exception {
        return fonts(c).set(new FontDescription.Resource(Identifier.parse(c.get("font").getAsString())));
    }

    static void glyphs(JsonObject c, JsonObject result) throws Exception {
        var fonts = fonts(c);
        var source = set(c).source(flag(c, "fishy"));
        var list = new JsonArray();
        for (int cp = c.get("from").getAsInt(); cp <= c.get("to").getAsInt(); cp++) {
            BakedGlyph g = source.getGlyph(cp);
            var e = new JsonArray();
            e.add(cp);
            e.add(g.info().getAdvance());
            e.add(g.info().getBoldOffset());
            e.add(g.info().getShadowOffset());
            if (g instanceof BakedSheetGlyph s) {
                if (fonts.texture(s).equals("missing")) continue;
                e.add(fonts.texture(s));
                e.add(fonts.colored(s));
                for (String k : new String[] {"left", "right", "up", "down"}) e.add((float) Stub.get(s, BakedSheetGlyph.class, k));
            } else {
                e.add(JsonNull.INSTANCE);
            }
            list.add(e);
        }
        result.add("glyphs", list);
        result.add("draws", new JsonArray());
    }

    @SuppressWarnings("unchecked")
    static void byWidth(JsonObject c, JsonObject result) throws Exception {
        var map = (Map<Integer, List<Integer>>) Stub.get(set(c), FontSet.class, "glyphsByWidth");
        var out = new JsonObject();
        map.forEach((w, list) -> {
            var a = new JsonArray();
            list.forEach(a::add);
            out.add(String.valueOf(w), a);
        });
        result.add("by_width", out);
        result.add("draws", new JsonArray());
    }

    public static void main(String[] args) throws Exception {
        GuiGame.init();
        var gson = new Gson();
        var out = new StringBuilder();
        for (String line : Files.readAllLines(Path.of(args[1]))) {
            if (line.isBlank()) continue;
            JsonObject c = JsonParser.parseString(line).getAsJsonObject();
            var result = new JsonObject();
            result.add("case", c);
            try {
                switch (c.get("kind").getAsString()) {
                    case "glyphs" -> glyphs(c, result);
                    case "bywidth" -> byWidth(c, result);
                    default -> text(c, result);
                }
            } catch (Throwable error) {
                var sw = new java.io.StringWriter();
                error.printStackTrace(new java.io.PrintWriter(sw));
                result = new JsonObject();
                result.add("case", c);
                result.addProperty("error", sw.toString().lines().limit(8).reduce("", (a, b) -> a + b + " | "));
            }
            out.append(gson.toJson((JsonElement) result)).append('\n');
        }
        Files.writeString(Path.of(args[0], "vanilla.jsonl"), out.toString(), StandardCharsets.UTF_8);
    }
}
