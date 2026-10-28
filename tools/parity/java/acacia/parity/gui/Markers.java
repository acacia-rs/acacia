package acacia.parity.gui;

import com.google.gson.JsonArray;
import com.google.gson.JsonNull;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.inventory.tooltip.ClientBundleTooltip;
import net.minecraft.client.gui.screens.inventory.tooltip.ClientTextTooltip;
import net.minecraft.client.gui.screens.inventory.tooltip.ClientTooltipComponent;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.client.renderer.state.MapRenderState;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.DyeColor;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.ItemStackTemplate;
import net.minecraft.world.item.component.BundleContents;
import net.minecraft.world.level.block.entity.BannerPatternLayers;
import org.joml.Quaternionfc;
import org.joml.Vector3fc;

/**
 * GuiCapture's marker calls: pictures-in-picture (an entity, the enchanting book, a banner, a map)
 * and tooltip images other than a bundle's, recorded by their parameters instead of drawn. Their
 * draw rules belong to other harnesses (entity, item and map rendering).
 */
final class Markers {
    private Markers() {}

    static JsonArray floats(float... values) {
        var a = new JsonArray();
        for (float v : values) a.add(v);
        return a;
    }

    static void box(com.google.gson.JsonObject o, int x0, int y0, int x1, int y1) {
        o.addProperty("x0", x0); o.addProperty("y0", y0); o.addProperty("x1", x1); o.addProperty("y1", y1);
    }

    static void entity(GuiCapture g, EntityRenderState state, float scale, Vector3fc translation, Quaternionfc rotation,
                       Quaternionfc camera, int x0, int y0, int x1, int y1) {
        var o = GuiCapture.call("entity");
        o.addProperty("entity", BuiltInRegistries.ENTITY_TYPE.getKey(state.entityType).toString());
        o.addProperty("scale", scale);
        o.add("translation", floats(translation.x(), translation.y(), translation.z()));
        o.add("rotation", floats(rotation.x(), rotation.y(), rotation.z(), rotation.w()));
        o.add("camera", camera == null ? JsonNull.INSTANCE : floats(camera.x(), camera.y(), camera.z(), camera.w()));
        if (state instanceof LivingEntityRenderState living) {
            o.add("angles", floats(living.bodyRot, living.yRot, living.xRot));
        }
        box(o, x0, y0, x1, y1);
        g.posed(o, g.stack);
    }

    static void book(GuiCapture g, Identifier texture, float scale, float open, float flip, int x0, int y0, int x1, int y1) {
        var o = GuiCapture.call("book");
        o.addProperty("texture", texture.getPath());
        o.add("book", floats(scale, open, flip));
        box(o, x0, y0, x1, y1);
        g.posed(o, g.stack);
    }

    static void banner(GuiCapture g, DyeColor base, BannerPatternLayers layers, int x0, int y0, int x1, int y1) {
        var o = GuiCapture.call("banner");
        o.addProperty("base", base.getSerializedName());
        var patterns = new JsonArray();
        for (var layer : layers.layers()) {
            patterns.add(layer.pattern().unwrapKey().map(k -> k.identifier().toString()).orElse("?") + " " + layer.color().getSerializedName());
        }
        o.add("patterns", patterns);
        box(o, x0, y0, x1, y1);
        g.posed(o, g.stack);
    }

    static void map(GuiCapture g, MapRenderState state) {
        var o = GuiCapture.call("map");
        o.addProperty("decorations", state.decorations.size());
        g.posed(o, g.stack);
    }

    /** The texture of the sprite getSprite last handed out (the loom's pattern icons blit it next). */
    static Identifier lastSprite;

    /**
     * A stand-in atlas sprite spanning UVs 0 to 1, so what a screen blits from it is in the sprite's
     * own UVs (the atlas is not stitched here).
     */
    static net.minecraft.client.renderer.texture.TextureAtlasSprite sprite(GuiCapture g, net.minecraft.client.resources.model.sprite.SpriteId id) {
        try {
            var s = Stub.alloc(net.minecraft.client.renderer.texture.TextureAtlasSprite.class);
            var type = net.minecraft.client.renderer.texture.TextureAtlasSprite.class;
            Stub.put(s, type, "atlasLocation", id.atlasLocation());
            Stub.put(s, type, "u0", 0.0F); Stub.put(s, type, "u1", 1.0F);
            Stub.put(s, type, "v0", 0.0F); Stub.put(s, type, "v1", 1.0F);
            lastSprite = id.texture();
            return s;
        } catch (Exception e) {
            throw new IllegalStateException(e);
        }
    }

    /** blit(atlas, x0, y0, x1, y1, u0, u1, v0, v1) of the last sprite handed out, its UVs sprite-relative. */
    static void spriteBlit(GuiCapture g, Identifier atlas, int x0, int y0, int x1, int y1, float u0, float u1, float v0, float v1) {
        var o = GuiCapture.call("sprite_blit");
        o.addProperty("atlas", atlas.toString());
        o.addProperty("sprite", String.valueOf(lastSprite));
        box(o, x0, y0, x1, y1);
        o.add("uv", floats(u0, u1, v0, v1));
        g.posed(o, g.stack);
    }

    /** Text lines as they are; any other line becomes a marker of its size where its image would be drawn. */
    static List<ClientTooltipComponent> tooltipImages(GuiCapture g, List<ClientTooltipComponent> lines) {
        var out = new ArrayList<ClientTooltipComponent>(lines.size());
        for (var line : lines) {
            if (line instanceof ClientTextTooltip) {
                out.add(line);
                continue;
            }
            if (line instanceof ClientBundleTooltip bundle) {
                out.add(bundleKeyed(g, bundle));
                continue;
            }
            out.add(new ClientTooltipComponent() {
                @Override public int getHeight(Font font) { return line.getHeight(font); }
                @Override public int getWidth(Font font) { return line.getWidth(font); }
                @Override public boolean showTooltipWithItemInHand() { return line.showTooltipWithItemInHand(); }
                @Override public void extractText(GuiGraphicsExtractor graphics, Font font, int x, int y) { line.extractText(graphics, font, x, y); }
                @Override public void extractImage(Font font, int x, int y, int w, int h, GuiGraphicsExtractor graphics) {
                    var o = GuiCapture.call("tooltip_image");
                    o.addProperty("kind", line.getClass().getSimpleName());
                    o.addProperty("x", x); o.addProperty("y", y);
                    o.addProperty("w", line.getWidth(font)); o.addProperty("h", line.getHeight(font));
                    g.posed(o, g.stack);
                }
            });
        }
        return out;
    }

    /** The first content key: a bundle's item `i` (by ItemStack.matches, so equal stacks share it) is `CONTENT_KEY + i`. */
    static final int CONTENT_KEY = 10000;

    /** A bundle image drawn for real, the stacks it creates keyed by their place in its contents. */
    static ClientTooltipComponent bundleKeyed(GuiCapture g, ClientBundleTooltip line) {
        return new ClientTooltipComponent() {
            @Override public int getHeight(Font font) { return line.getHeight(font); }
            @Override public int getWidth(Font font) { return line.getWidth(font); }
            @Override public boolean showTooltipWithItemInHand() { return line.showTooltipWithItemInHand(); }
            @Override public void extractImage(Font font, int x, int y, int w, int h, GuiGraphicsExtractor graphics) {
                BundleContents contents;
                try {
                    contents = (BundleContents) Stub.get(line, ClientBundleTooltip.class, "contents");
                } catch (Exception e) {
                    throw new IllegalStateException(e);
                }
                var items = contents.items().stream().map(ItemStackTemplate::create).toList();
                var outer = g.keyOf;
                g.keyOf = s -> {
                    for (int i = 0; i < items.size(); i++) if (ItemStack.matches(items.get(i), s)) return CONTENT_KEY + i;
                    return outer.applyAsInt(s);
                };
                try {
                    line.extractImage(font, x, y, w, h, graphics);
                } finally {
                    g.keyOf = outer;
                }
            }
        };
    }
}
