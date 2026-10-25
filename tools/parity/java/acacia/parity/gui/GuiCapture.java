package acacia.parity.gui;

import com.google.gson.JsonArray;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import java.util.IdentityHashMap;
import java.util.Objects;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.ActiveTextCollector;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.TextAlignment;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.FontDescription;
import net.minecraft.network.chat.Style;
import net.minecraft.resources.Identifier;
import net.minecraft.util.ARGB;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.item.ItemStack;
import org.joml.Matrix3x2f;
import org.joml.Matrix3x2fStack;
import org.joml.Matrix3x2fc;

/**
 * A GuiGraphicsExtractor that records its calls as JSON instead of drawing: sprite and texture
 * blits, fills, gradients, text (string and style runs), items (by the key the oracle gave the
 * stack), item decorations, scissors and scrolling text, each with its pose when that is not the
 * identity. The Rust side writes the same objects (crates/acacia-ui/src/java/encode.rs).
 * Adapted from Horizon's VanillaHudOracle.Capture (MIT, (c) Ludovic Marie).
 */
public class GuiCapture extends GuiGraphicsExtractor {
    public JsonArray draws = new JsonArray();
    public Matrix3x2fStack stack = new Matrix3x2fStack(16);
    public int width;
    public int height;
    public Font font;
    /** The key each stack is recorded under (a slot number); -1 for others. */
    public IdentityHashMap<ItemStack, Integer> keys = new IdentityHashMap<>();
    /** The scissor stack, as GuiGraphicsExtractor.ScissorStack keeps it (widgets hover through it). */
    java.util.ArrayDeque<net.minecraft.client.gui.navigation.ScreenRectangle> scissors = new java.util.ArrayDeque<>();
    /** Off: itemDecorations is recorded as one call; on: as the fills and text it makes. */
    public boolean realDecorations;

    private GuiCapture() {
        super(null, null, 0, 0);
    }

    /** A capture for a `width` x `height` GUI drawing for `mc` (whose font measures). */
    public static GuiCapture create(Minecraft mc, int width, int height) throws Exception {
        var c = Stub.alloc(GuiCapture.class);
        c.draws = new JsonArray();
        c.stack = new Matrix3x2fStack(16);
        c.keys = new IdentityHashMap<>();
        c.scissors = new java.util.ArrayDeque<>();
        c.width = width;
        c.height = height;
        c.font = mc.font;
        Stub.put(c, GuiGraphicsExtractor.class, "minecraft", mc);
        Stub.put(c, GuiGraphicsExtractor.class, "pose", c.stack);
        return c;
    }

    @Override public int guiWidth() { return width; }
    @Override public int guiHeight() { return height; }
    @Override public Matrix3x2fStack pose() { return stack; }
    @Override public void nextStratum() { draws.add(call("stratum")); }
    @Override public void blurBeforeThisStratum() { draws.add(call("blur")); }

    static JsonArray matrix(Matrix3x2fc m) {
        var a = new JsonArray();
        for (float v : new float[] {m.m00(), m.m01(), m.m10(), m.m11(), m.m20(), m.m21()}) a.add(v);
        return a;
    }

    /** Adds `o` with the pose `m` when that is not the identity. */
    JsonObject posed(JsonObject o, Matrix3x2fc m) {
        if (!m.equals(new Matrix3x2f())) o.add("matrix", matrix(m));
        draws.add(o);
        return o;
    }

    static JsonObject call(String type) {
        var o = new JsonObject();
        o.addProperty("type", type);
        return o;
    }

    void sprite(RenderPipeline p, Identifier id, int x, int y, int w, int h, int[] region, int color) {
        var o = call("sprite");
        o.addProperty("sprite", id.getPath());
        o.addProperty("x", x); o.addProperty("y", y); o.addProperty("w", w); o.addProperty("h", h);
        if (region != null) {
            var r = new JsonArray();
            for (int v : region) r.add(v);
            o.add("region", r);
        } else {
            o.add("region", JsonNull.INSTANCE);
        }
        o.addProperty("color", color);
        o.addProperty("pipeline", p.getLocation().getPath());
        posed(o, stack);
    }

    @Override public void blitSprite(RenderPipeline p, Identifier id, int x, int y, int w, int h, int color) { sprite(p, id, x, y, w, h, null, color); }
    @Override public void blitSprite(RenderPipeline p, Identifier id, int sw, int sh, int u, int v, int x, int y, int w, int h, int color) {
        sprite(p, id, x, y, w, h, new int[] {sw, sh, u, v}, color);
    }
    @Override public void blitSprite(RenderPipeline p, net.minecraft.client.renderer.texture.TextureAtlasSprite s, int x, int y, int w, int h, int color) {
        var o = call("atlas");
        o.addProperty("sprite", s.contents().name().getPath());
        o.addProperty("x", x); o.addProperty("y", y); o.addProperty("w", w); o.addProperty("h", h);
        o.addProperty("color", color);
        posed(o, stack);
    }
    @Override public void blit(RenderPipeline p, Identifier texture, int x, int y, float u, float v, int w, int h, int srcW, int srcH, int texW, int texH, int color) {
        var o = call("blit");
        o.addProperty("texture", texture.getPath());
        o.addProperty("pipeline", p.getLocation().getPath());
        o.addProperty("x", x); o.addProperty("y", y); o.addProperty("w", w); o.addProperty("h", h);
        var uv = new JsonArray(); uv.add(u); uv.add(v); o.add("uv", uv);
        var src = new JsonArray(); src.add(srcW); src.add(srcH); o.add("src", src);
        var tex = new JsonArray(); tex.add(texW); tex.add(texH); o.add("tex", tex);
        o.addProperty("color", color);
        posed(o, stack);
    }
    @Override public void fill(RenderPipeline p, int x0, int y0, int x1, int y1, int color) {
        var o = call("fill");
        o.addProperty("x0", Math.min(x0, x1)); o.addProperty("y0", Math.min(y0, y1));
        o.addProperty("x1", Math.max(x0, x1)); o.addProperty("y1", Math.max(y0, y1));
        o.addProperty("color", color);
        if (p != net.minecraft.client.renderer.RenderPipelines.GUI) o.addProperty("pipeline", p.getLocation().getPath());
        posed(o, stack);
    }
    @Override public void fillGradient(int x0, int y0, int x1, int y1, int top, int bottom) {
        var o = call("gradient");
        o.addProperty("x0", x0); o.addProperty("y0", y0); o.addProperty("x1", x1); o.addProperty("y1", y1);
        o.addProperty("top", top); o.addProperty("bottom", bottom);
        posed(o, stack);
    }
    @Override public void enableScissor(int x0, int y0, int x1, int y1) {
        var o = call("scissor");
        o.addProperty("x0", x0); o.addProperty("y0", y0); o.addProperty("x1", x1); o.addProperty("y1", y1);
        posed(o, stack);
        pushScissor(x0, y0, x1, y1);
    }
    @Override public void disableScissor() {
        if (!scissors.isEmpty()) scissors.removeLast();
        draws.add(call("unscissor"));
    }
    void pushScissor(int x0, int y0, int x1, int y1) {
        var r = new net.minecraft.client.gui.navigation.ScreenRectangle(x0, y0, x1 - x0, y1 - y0).transformAxisAligned(stack);
        var last = Objects.requireNonNullElse(scissors.peekLast(), new net.minecraft.client.gui.navigation.ScreenRectangle(0, 0, width, height));
        scissors.addLast(Objects.requireNonNullElse(r.intersection(last), net.minecraft.client.gui.navigation.ScreenRectangle.empty()));
    }
    // ScissorStack.containsPoint tests the first scissor pushed, not the innermost.
    @Override public boolean containsPointInScissor(int x, int y) { return scissors.isEmpty() || scissors.peekFirst().containsPoint(x, y); }

    /** Runs `[text, rgb|null, bold, italic, underlined, strikethrough, obfuscated, font|null]`; null when unstyled. */
    static JsonArray runs(FormattedCharSequence str, StringBuilder plain) {
        var runs = new JsonArray();
        boolean[] styled = {false};
        str.accept((index, style, cp) -> {
            plain.appendCodePoint(cp);
            var run = new JsonArray();
            run.add(new String(Character.toChars(cp)));
            if (style.getColor() == null) run.add(JsonNull.INSTANCE); else run.add(style.getColor().getValue());
            run.add(style.isBold()); run.add(style.isItalic()); run.add(style.isUnderlined());
            run.add(style.isStrikethrough()); run.add(style.isObfuscated());
            String font = style.getFont() instanceof FontDescription.Resource r && !r.id().equals(Identifier.withDefaultNamespace("default"))
                ? r.id().toString() : null;
            if (font == null) run.add(JsonNull.INSTANCE); else run.add(font);
            styled[0] |= !run.get(1).isJsonNull() || style.isBold() || style.isItalic() || style.isUnderlined() || style.isStrikethrough()
                || style.isObfuscated() || font != null;
            JsonArray last = runs.isEmpty() ? null : runs.get(runs.size() - 1).getAsJsonArray();
            boolean same = last != null;
            for (int i = 1; same && i < 8; i++) same = Objects.equals(last.get(i), run.get(i));
            if (same) last.set(0, new JsonPrimitive(last.get(0).getAsString() + run.get(0).getAsString()));
            else runs.add(run);
            return true;
        });
        return styled[0] ? runs : null;
    }

    void text(FormattedCharSequence str, int x, int y, int color, boolean shadow, Matrix3x2fc pose) {
        var o = call("text");
        var plain = new StringBuilder();
        var runs = runs(str, plain);
        o.addProperty("text", plain.toString());
        o.addProperty("x", x); o.addProperty("y", y); o.addProperty("color", color); o.addProperty("shadow", shadow);
        if (runs != null) o.add("runs", runs);
        posed(o, pose);
    }

    @Override public void text(Font font, FormattedCharSequence str, int x, int y, int color, boolean shadow) {
        if (ARGB.alpha(color) != 0) text(str, x, y, color, shadow, stack);
    }

    /** Text collectors record what they would add: text at their parameters' pose and opacity. */
    ActiveTextCollector collector(float opacity) {
        var self = this;
        return new ActiveTextCollector() {
            Parameters parameters = new Parameters(new Matrix3x2f(stack), opacity, null);
            @Override public Parameters defaultParameters() { return parameters; }
            @Override public void defaultParameters(Parameters p) { parameters = p; }
            @Override public void accept(TextAlignment alignment, int anchorX, int y, Parameters p, FormattedCharSequence text) {
                int left = alignment.calculateLeft(anchorX, self.font, text);
                if (ARGB.as8BitChannel(p.opacity()) != 0) self.text(text, left, y, ARGB.white(p.opacity()), true, p.pose());
            }
            @Override public void acceptScrolling(Component m, int cx, int l, int r, int t, int b, Parameters p) {
                var o = call("scrolling");
                var plain = new StringBuilder();
                var runs = runs(m.getVisualOrderText(), plain);
                o.addProperty("text", plain.toString());
                if (runs != null) o.add("runs", runs);
                o.addProperty("centerX", cx); o.addProperty("x0", l); o.addProperty("x1", r); o.addProperty("y0", t); o.addProperty("y1", b);
                o.addProperty("color", ARGB.white(p.opacity()));
                self.posed(o, p.pose());
            }
        };
    }

    @Override public ActiveTextCollector textRenderer(HoveredTextEffects effects, java.util.function.Consumer<Style> consumer) { return collector(1.0F); }
    @Override public ActiveTextCollector textRendererForWidget(AbstractWidget owner, HoveredTextEffects effects) { return collector(owner.getAlpha()); }

    void recordItem(ItemStack stack, int x, int y, int seed) {
        if (stack.isEmpty()) return;
        var o = call("item");
        o.addProperty("slot", keys.getOrDefault(stack, -1));
        o.addProperty("x", x); o.addProperty("y", y); o.addProperty("seed", seed);
        o.add("matrix", matrix(this.stack));
        draws.add(o);
    }
    @Override public void item(ItemStack stack, int x, int y) { recordItem(stack, x, y, 0); }
    @Override public void item(ItemStack stack, int x, int y, int seed) { recordItem(stack, x, y, seed); }
    @Override public void item(LivingEntity owner, ItemStack stack, int x, int y, int seed) { recordItem(stack, x, y, seed); }
    @Override public void fakeItem(ItemStack stack, int x, int y) { recordItem(stack, x, y, 0); }
    @Override public void fakeItem(ItemStack stack, int x, int y, int seed) { recordItem(stack, x, y, seed); }

    @Override public void itemDecorations(Font font, ItemStack stack, int x, int y, String count) {
        if (realDecorations) {
            super.itemDecorations(font, stack, x, y, count);
        } else if (!stack.isEmpty()) {
            var o = call("decorations");
            o.addProperty("slot", keys.getOrDefault(stack, -1));
            o.addProperty("x", x); o.addProperty("y", y);
            if (count == null) o.add("count", JsonNull.INSTANCE); else o.addProperty("count", count);
            posed(o, this.stack);
        }
    }
}
