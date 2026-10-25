package acacia.parity.gui;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.layouts.EqualSpacingLayout;
import net.minecraft.client.gui.layouts.FrameLayout;
import net.minecraft.client.gui.layouts.GridLayout;
import net.minecraft.client.gui.layouts.Layout;
import net.minecraft.client.gui.layouts.LayoutElement;
import net.minecraft.client.gui.layouts.LayoutSettings;
import net.minecraft.client.gui.layouts.LinearLayout;
import net.minecraft.client.gui.layouts.SpacerElement;
import net.minecraft.network.chat.Component;

/**
 * The `layout` kind: a tree of vanilla layouts (grid, row-helper grid, linear, frame, equal
 * spacing) over buttons and spacers, arranged, aligned in the case's rectangle and its buttons
 * drawn. Node schema: cases.py `layout_node`.
 */
final class WidgetsLayouts {
    private WidgetsLayouts() {}

    static int buttons;

    static LayoutSettings settings(LayoutSettings base, JsonObject child) {
        if (!child.has("pad")) return base;
        var p = child.getAsJsonArray("pad");
        var a = child.getAsJsonArray("align");
        return base.padding(p.get(0).getAsInt(), p.get(1).getAsInt(), p.get(2).getAsInt(), p.get(3).getAsInt())
            .align(a.get(0).getAsFloat(), a.get(1).getAsFloat());
    }

    static LayoutElement elem(JsonObject child) {
        if (child.has("button")) {
            var s = child.getAsJsonArray("button");
            return Button.builder(Component.literal("B" + buttons++), b -> {}).size(s.get(0).getAsInt(), s.get(1).getAsInt()).build();
        }
        if (child.has("spacer")) {
            var s = child.getAsJsonArray("spacer");
            return new SpacerElement(s.get(0).getAsInt(), s.get(1).getAsInt());
        }
        return node(child.getAsJsonObject("layout"));
    }

    static Layout node(JsonObject n) {
        JsonArray children = n.getAsJsonArray("children");
        switch (n.get("type").getAsString()) {
            case "grid" -> {
                var grid = new GridLayout().rowSpacing(n.get("rowSpacing").getAsInt()).columnSpacing(n.get("colSpacing").getAsInt());
                for (var e : children) {
                    var c = e.getAsJsonObject();
                    grid.addChild(elem(c), c.get("row").getAsInt(), c.get("col").getAsInt(), c.get("rows").getAsInt(), c.get("cols").getAsInt(),
                        settings(grid.newCellSettings(), c));
                }
                return grid;
            }
            case "helper" -> {
                var grid = new GridLayout().rowSpacing(n.get("rowSpacing").getAsInt()).columnSpacing(n.get("colSpacing").getAsInt());
                var helper = grid.createRowHelper(n.get("columns").getAsInt());
                for (var e : children) {
                    var c = e.getAsJsonObject();
                    helper.addChild(elem(c), c.get("span").getAsInt(), settings(grid.newCellSettings(), c));
                }
                return grid;
            }
            case "linear" -> {
                var linear = (n.get("vertical").getAsBoolean() ? LinearLayout.vertical() : LinearLayout.horizontal()).spacing(n.get("spacing").getAsInt());
                for (var e : children) {
                    var c = e.getAsJsonObject();
                    linear.addChild(elem(c), settings(linear.newCellSettings(), c));
                }
                return linear;
            }
            case "frame" -> {
                var frame = new FrameLayout(n.get("minW").getAsInt(), n.get("minH").getAsInt());
                for (var e : children) {
                    var c = e.getAsJsonObject();
                    frame.addChild(elem(c), settings(frame.newChildLayoutSettings(), c));
                }
                return frame;
            }
            default -> {
                var equal = new EqualSpacingLayout(n.get("w").getAsInt(), n.get("h").getAsInt(),
                    n.get("vertical").getAsBoolean() ? EqualSpacingLayout.Orientation.VERTICAL : EqualSpacingLayout.Orientation.HORIZONTAL);
                for (var e : children) {
                    var c = e.getAsJsonObject();
                    equal.addChild(elem(c), settings(equal.newChildLayoutSettings(), c));
                }
                return equal;
            }
        }
    }

    static void render(Minecraft mc, JsonObject c, GuiCapture capture, int mx, int my) {
        buttons = 0;
        var root = node(c.getAsJsonObject("layout"));
        root.arrangeElements();
        var r = c.getAsJsonArray("rect");
        FrameLayout.alignInRectangle(root, r.get(0).getAsInt(), r.get(1).getAsInt(), r.get(2).getAsInt(), r.get(3).getAsInt(),
            r.get(4).getAsFloat(), r.get(5).getAsFloat());
        root.visitWidgets(w -> w.extractRenderState(capture, mx, my, 0.0F));
    }
}
