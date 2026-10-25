package acacia.parity.gui;

import com.google.gson.JsonArray;
import com.google.gson.JsonNull;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.inventory.tooltip.ClientTextTooltip;
import net.minecraft.client.gui.screens.inventory.tooltip.ClientTooltipComponent;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.client.renderer.state.MapRenderState;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.DyeColor;
import net.minecraft.world.level.block.entity.BannerPatternLayers;
import org.joml.Quaternionfc;
import org.joml.Vector3fc;

/**
 * GuiCapture's marker calls: pictures-in-picture (an entity, the enchanting book, a banner, a map)
 * and tooltip images, recorded by their parameters instead of drawn. Their draw rules belong to
 * other harnesses (entity, item and map rendering; bundle tooltips' own kind).
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

    /** Text lines as they are; any other line becomes a marker of its size where its image would be drawn. */
    static List<ClientTooltipComponent> tooltipImages(GuiCapture g, List<ClientTooltipComponent> lines) {
        var out = new ArrayList<ClientTooltipComponent>(lines.size());
        for (var line : lines) {
            if (line instanceof ClientTextTooltip) {
                out.add(line);
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
}
