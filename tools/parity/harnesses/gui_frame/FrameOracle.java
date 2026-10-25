package acacia.parity.gui;

import acacia.parity.Atlases;
import acacia.parity.Client;
import acacia.parity.Gui;
import acacia.parity.Out;
import acacia.parity.Shell;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import java.lang.reflect.Modifier;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import net.minecraft.client.gui.ActiveTextCollector;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.FontDescription;
import net.minecraft.network.chat.MutableComponent;
import net.minecraft.network.chat.Style;
import net.minecraft.network.chat.TextColor;
import net.minecraft.resources.Identifier;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.util.Util;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

/**
 * The `gui_frame` harness's oracle: each GUI harness's recorded calls (GuiCapture's JSON, read
 * from its vanilla.jsonl) replayed onto a real GuiGraphicsExtractor at the case's GUI size (GUI
 * scale 1), then what GuiRenderer draws in its order (`Gui.frame`), cut down to the fields the
 * comparison reads. Items draw stone (only their rect is compared); decorations and picture
 * markers are left out. Util's clock stands at MILLIS. args: out dir, then `name=vanilla.jsonl`...
 */
public final class FrameOracle {
    static final long MILLIS = 1_234_567L;
    static final Set<String> KEPT = Set.of("kind", "afterBlur", "pipeline", "pose", "scissorArea", "textureSetup", "texure0",
        "x0", "y0", "x1", "y1", "u0", "u1", "v0", "v1", "color", "col1", "col2", "tileWidth", "tileHeight", "renderable",
        "x", "y", "shadowColor", "shadowOffset", "boldOffset", "glyph", "left", "right", "up", "down", "style", "bold", "italic",
        "position", "width", "height");
    static final Map<String, RenderPipeline> PIPELINES = new HashMap<>();

    public static void main(String[] args) throws Exception {
        var mc = Client.minecraft();
        GuiGame.init();
        Shell.set(mc, "font", GuiGame.font);
        Shell.set(Util.class, "timeSource", (net.minecraft.util.TimeSource.NanoTimeSource) () -> MILLIS * 1_000_000L);
        for (var f : RenderPipelines.class.getFields()) {
            if (Modifier.isStatic(f.getModifiers()) && f.get(null) instanceof RenderPipeline p) PIPELINES.put(p.getLocation().getPath(), p);
        }
        var out = Path.of(args[0]);
        var blends = new TreeMap<String, Object>();
        PIPELINES.forEach((path, p) -> blends.put(path, p.getColorTargetStates().toString()));
        Out.json(out.resolve("blends.json"), blends);
        for (int a = 1; a < args.length; a++) {
            var named = args[a].split("=", 2);
            try (var lines = new Out.Lines(out.resolve(named[0] + ".jsonl"))) {
                for (var line : Files.readAllLines(Path.of(named[1]))) {
                    if (!line.isBlank()) lines.write(frame(JsonParser.parseString(line).getAsJsonObject()));
                }
            }
        }
    }

    static Map<String, Object> frame(JsonObject recorded) {
        if (recorded.has("error")) return Out.map("error", "oracle: " + recorded.get("error").getAsString());
        var c = recorded.getAsJsonObject("case");
        try {
            int width = int_(c, "width", 427), height = int_(c, "height", 240);
            Client.window(width, height, 1);
            var frame = Gui.frame(0, 0, g -> replay(g, recorded.getAsJsonArray("draws")));
            var draws = new ArrayList<Object>();
            for (var d : (List<?>) frame.get("draws")) draws.add(slim(d));
            return Out.map("draws", draws, "window", List.of(width, height));
        } catch (Throwable t) {
            return Out.map("error", t.toString());
        }
    }

    static int int_(JsonObject o, String key, int fallback) {
        return absent(o, key) ? fallback : o.get(key).getAsInt();
    }

    /** Gson leaves null members out of the oracles' lines. */
    static boolean absent(JsonObject o, String key) {
        return !o.has(key) || o.get(key).isJsonNull();
    }

    @SuppressWarnings("unchecked")
    static Object slim(Object tree) {
        if (tree instanceof Map<?, ?> m) {
            var out = new TreeMap<String, Object>();
            ((Map<String, Object>) m).forEach((k, v) -> { if (KEPT.contains(k)) out.put(k, slim(v)); });
            return out;
        }
        return tree;
    }

    static void replay(GuiGraphicsExtractor g, JsonArray draws) {
        try {
            for (var e : draws) call(g, e.getAsJsonObject());
        } catch (Exception ex) {
            throw new RuntimeException(ex);
        }
    }

    static void call(GuiGraphicsExtractor g, JsonObject d) throws Exception {
        var m = d.has("matrix") ? d.getAsJsonArray("matrix") : null;
        if (m != null) g.pose().set(f(m, 0), f(m, 1), f(m, 2), f(m, 3), f(m, 4), f(m, 5));
        else g.pose().identity();
        switch (d.get("type").getAsString()) {
            case "sprite" -> {
                var id = Identifier.withDefaultNamespace(d.get("sprite").getAsString());
                var p = pipeline(d, "pipeline/gui_textured");
                if (absent(d, "region")) g.blitSprite(p, id, i(d, "x"), i(d, "y"), i(d, "w"), i(d, "h"), i(d, "color"));
                else {
                    var r = d.getAsJsonArray("region");
                    g.blitSprite(p, id, r.get(0).getAsInt(), r.get(1).getAsInt(), r.get(2).getAsInt(), r.get(3).getAsInt(),
                        i(d, "x"), i(d, "y"), i(d, "w"), i(d, "h"), i(d, "color"));
                }
            }
            case "atlas" -> g.blitSprite(RenderPipelines.GUI_TEXTURED, atlasSprite(d.get("sprite").getAsString()), i(d, "x"), i(d, "y"), i(d, "w"), i(d, "h"), i(d, "color"));
            case "blit" -> {
                var uv = d.getAsJsonArray("uv");
                var src = d.getAsJsonArray("src");
                var tex = d.getAsJsonArray("tex");
                g.blit(pipeline(d, "pipeline/gui_textured"), Identifier.withDefaultNamespace(d.get("texture").getAsString()), i(d, "x"), i(d, "y"),
                    uv.get(0).getAsFloat(), uv.get(1).getAsFloat(), i(d, "w"), i(d, "h"), src.get(0).getAsInt(), src.get(1).getAsInt(),
                    tex.get(0).getAsInt(), tex.get(1).getAsInt(), i(d, "color"));
            }
            case "fill" -> g.fill(pipeline(d, "pipeline/gui"), i(d, "x0"), i(d, "y0"), i(d, "x1"), i(d, "y1"), i(d, "color"));
            case "gradient" -> g.fillGradient(i(d, "x0"), i(d, "y0"), i(d, "x1"), i(d, "y1"), i(d, "top"), i(d, "bottom"));
            case "scissor" -> g.enableScissor(i(d, "x0"), i(d, "y0"), i(d, "x1"), i(d, "y1"));
            case "unscissor" -> g.disableScissor();
            case "stratum" -> g.nextStratum();
            case "blur" -> g.blurBeforeThisStratum();
            case "text" -> g.text(GuiGame.font, sequence(d), i(d, "x"), i(d, "y"), i(d, "color"), d.get("shadow").getAsBoolean());
            case "scrolling" -> {
                var collector = g.textRenderer();
                var p = collector.defaultParameters();
                float opacity = ((i(d, "color") >>> 24) + 0.5F) / 255.0F;
                var params = new ActiveTextCollector.Parameters(p.pose(), opacity, p.scissor());
                // The centred branch of defaultScrollingHelper reads the collector's own parameters.
                collector.defaultParameters(params);
                collector.acceptScrolling(component(d), i(d, "centerX"), i(d, "x0"), i(d, "x1"), i(d, "y0"), i(d, "y1"), params);
            }
            case "item" -> g.item(new ItemStack(Items.STONE), i(d, "x"), i(d, "y"), i(d, "seed"));
            default -> {}
        }
    }

    static float f(JsonArray a, int n) {
        return a.get(n).getAsFloat();
    }

    static int i(JsonObject d, String key) {
        return d.get(key).getAsInt();
    }

    static RenderPipeline pipeline(JsonObject d, String fallback) {
        var path = absent(d, "pipeline") ? fallback : d.get("pipeline").getAsString();
        var p = PIPELINES.get(path);
        if (p == null) throw new IllegalArgumentException("no pipeline " + path);
        return p;
    }

    static TextureAtlasSprite atlasSprite(String path) throws Exception {
        var id = Identifier.withDefaultNamespace(path);
        for (var atlas : Atlases.byTexture().values()) {
            var s = atlas.getSprite(id);
            if (s.contents().name().equals(id)) return s;
        }
        throw new IllegalArgumentException("no atlas has " + path);
    }

    /** `[text, rgb|null, bold, italic, underlined, strikethrough, obfuscated, font|null]` runs (GuiCapture.runs), or the plain text. */
    static List<Map.Entry<String, Style>> runs(JsonObject d) {
        var out = new ArrayList<Map.Entry<String, Style>>();
        if (!d.has("runs")) {
            out.add(Map.entry(d.get("text").getAsString(), Style.EMPTY));
            return out;
        }
        for (JsonElement e : d.getAsJsonArray("runs")) {
            var r = e.getAsJsonArray();
            var style = Style.EMPTY.withBold(r.get(2).getAsBoolean()).withItalic(r.get(3).getAsBoolean()).withUnderlined(r.get(4).getAsBoolean())
                .withStrikethrough(r.get(5).getAsBoolean()).withObfuscated(r.get(6).getAsBoolean());
            if (r.size() > 1 && !r.get(1).isJsonNull()) style = style.withColor(TextColor.fromRgb(r.get(1).getAsInt()));
            if (r.size() > 7 && !r.get(7).isJsonNull()) style = style.withFont(new FontDescription.Resource(Identifier.parse(r.get(7).getAsString())));
            if (r.size() > 8) style = style.withShadowColor(r.get(8).getAsInt());
            out.add(Map.entry(r.get(0).getAsString(), style));
        }
        return out;
    }

    static FormattedCharSequence sequence(JsonObject d) {
        var parts = new ArrayList<FormattedCharSequence>();
        for (var run : runs(d)) parts.add(FormattedCharSequence.forward(run.getKey(), run.getValue()));
        return FormattedCharSequence.composite(parts);
    }

    static Component component(JsonObject d) {
        MutableComponent out = Component.empty();
        for (var run : runs(d)) out.append(Component.literal(run.getKey()).withStyle(run.getValue()));
        return out;
    }
}
