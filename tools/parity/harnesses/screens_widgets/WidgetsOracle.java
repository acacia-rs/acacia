package acacia.parity.gui;

import static acacia.parity.gui.ScreenGame.b;
import static acacia.parity.gui.ScreenGame.component;
import static acacia.parity.gui.ScreenGame.f;
import static acacia.parity.gui.ScreenGame.i;
import static acacia.parity.gui.ScreenGame.s;

import com.google.gson.JsonObject;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.AbstractSliderButton;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.Checkbox;
import net.minecraft.client.gui.components.CycleButton;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.components.FocusableTextWidget;
import net.minecraft.client.gui.components.ImageButton;
import net.minecraft.client.gui.components.MultiLineTextWidget;
import net.minecraft.client.gui.components.SpriteIconButton;
import net.minecraft.client.gui.components.StringWidget;
import net.minecraft.client.gui.components.WidgetSprites;
import net.minecraft.client.gui.components.tabs.GridLayoutTab;
import net.minecraft.client.gui.components.tabs.MenuTabBar;
import net.minecraft.client.gui.components.tabs.Tab;
import net.minecraft.client.gui.components.tabs.TabManager;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.util.Util;

/**
 * The `screens_widgets` harness's oracle: each case builds one vanilla widget (or a layout of
 * buttons, a selection list, a tab bar) in the case's state on the stand-in client and records
 * its extractRenderState at the case's mouse position. Kinds and fields: cases.py.
 */
public final class WidgetsOracle {
    static Identifier id(String path) { return Identifier.withDefaultNamespace(path); }

    static WidgetSprites sprites(com.google.gson.JsonArray a) {
        return new WidgetSprites(id(a.get(0).getAsString()), id(a.get(1).getAsString()), id(a.get(2).getAsString()), id(a.get(3).getAsString()));
    }

    /** The state every widget kind shares: active, visible, alpha, focus (set without side effects). */
    static <W extends AbstractWidget> W common(W w, JsonObject c) throws Exception {
        w.active = b(c, "active", true);
        w.visible = b(c, "visible", true);
        w.setAlpha(f(c, "alpha", 1));
        Stub.put(w, AbstractWidget.class, "focused", b(c, "focused", false));
        return w;
    }

    static AbstractWidget widget(Minecraft mc, JsonObject c) throws Exception {
        int x = i(c, "x", 0), y = i(c, "y", 0), w = i(c, "w", 150), h = i(c, "h", 20);
        Component msg = component(c.get("message"));
        var font = mc.font;
        switch (c.get("kind").getAsString()) {
            case "button" -> {
                var button = Button.builder(msg, bt -> {}).bounds(x, y, w, h).build();
                if (c.has("highlight") && !c.get("highlight").isJsonNull()) {
                    boolean lit = c.get("highlight").getAsBoolean();
                    button.setOverrideRenderHighlightedSprite(() -> lit);
                }
                return common(button, c);
            }
            case "image" -> {
                return common(new ImageButton(x, y, w, h, sprites(c.getAsJsonArray("sprites")), bt -> {}, msg), c);
            }
            case "icon" -> {
                var size = c.getAsJsonArray("spriteSize");
                var offset = c.getAsJsonArray("spriteOffset");
                var button = SpriteIconButton.builder(msg, bt -> {}, b(c, "iconOnly", true)).size(w, h)
                    .sprite(sprites(c.getAsJsonArray("sprites")), size.get(0).getAsInt(), size.get(1).getAsInt())
                    .spriteOffset(offset.get(0).getAsInt(), offset.get(1).getAsInt()).build();
                button.setPosition(x, y);
                if (b(c, "loading", false)) button.setLoading(true);
                return common(button, c);
            }
            case "cycle" -> {
                var values = c.getAsJsonArray("values");
                var names = new java.util.ArrayList<Component>();
                var indices = new java.util.ArrayList<Integer>();
                for (int n = 0; n < values.size(); n++) { names.add(component(values.get(n))); indices.add(n); }
                String sprite = s(c, "sprite", null);
                var display = CycleButton.DisplayState.valueOf(s(c, "display", "NAME_AND_VALUE"));
                var button = CycleButton.<Integer>builder(v -> names.get(v), i(c, "value", 0)).withValues(indices).displayState(display)
                    .withSprite((bt, v) -> sprite == null ? null : id(sprite)).create(x, y, w, h, component(c.get("name")));
                return common(button, c);
            }
            case "checkbox" -> {
                var builder = Checkbox.builder(msg, font).pos(x, y).selected(b(c, "selected", false));
                if (c.has("maxWidth")) builder.maxWidth(i(c, "maxWidth", 0));
                return common(builder.build(), c);
            }
            case "slider" -> {
                var slider = new AbstractSliderButton(x, y, w, h, msg, c.get("value").getAsDouble()) {
                    @Override protected void updateMessage() { }
                    @Override protected void applyValue() { }
                };
                Stub.put(slider, AbstractSliderButton.class, "canChangeValue", b(c, "canChange", false));
                return common(slider, c);
            }
            case "editbox" -> { return EditBoxCase.build(mc, c, x, y, w, h, msg); }
            case "string" -> {
                var widget = new StringWidget(x, y, w, h, msg, font);
                if (i(c, "maxWidth", 0) > 0) widget.setMaxWidth(i(c, "maxWidth", 0), StringWidget.TextOverflow.valueOf(s(c, "overflow", "CLAMPED")));
                return common(widget, c);
            }
            case "multiline" -> {
                var widget = new MultiLineTextWidget(x, y, msg, font);
                if (c.has("maxWidth")) widget.setMaxWidth(i(c, "maxWidth", 0));
                if (c.has("maxRows")) widget.setMaxRows(i(c, "maxRows", 0));
                widget.setCentered(b(c, "centered", false));
                return common(widget, c);
            }
            case "focusable" -> {
                var widget = FocusableTextWidget.builder(msg, font, i(c, "padding", 4)).maxWidth(i(c, "maxWidth", -1))
                    .alwaysShowBorder(b(c, "alwaysBorder", true)).backgroundFill(FocusableTextWidget.BackgroundFill.valueOf(s(c, "fill", "ALWAYS"))).build();
                widget.setPosition(x, y);
                return common(widget, c);
            }
            case "tabs" -> {
                var manager = new TabManager(a -> {}, a -> {});
                var titles = c.getAsJsonArray("titles");
                var tabs = new Tab[titles.size()];
                for (int n = 0; n < tabs.length; n++) tabs[n] = new GridLayoutTab(component(titles.get(n)));
                var bar = MenuTabBar.builder(manager, c.get("width").getAsInt()).addTabs(tabs).build();
                manager.setCurrentTab(tabs[i(c, "selected", 0)], false);
                bar.arrangeElements(c.get("width").getAsInt());
                return bar;
            }
            default -> throw new IllegalArgumentException(c.get("kind").getAsString());
        }
    }

    public static void main(String[] args) throws Exception {
        ScreenGame.run(args, (c, result) -> {
            var mc = ScreenGame.create(i(c, "width", 427), i(c, "height", 240), b(c, "inWorld", true));
            var capture = GuiCapture.create(mc, i(c, "width", 427), i(c, "height", 240));
            var mouse = c.getAsJsonArray("mouse");
            int mx = mouse.get(0).getAsInt(), my = mouse.get(1).getAsInt();
            var scissor = c.has("scissor") ? c.getAsJsonArray("scissor") : null;
            if (scissor != null) capture.enableScissor(scissor.get(0).getAsInt(), scissor.get(1).getAsInt(), scissor.get(2).getAsInt(), scissor.get(3).getAsInt());
            switch (c.get("kind").getAsString()) {
                case "layout" -> WidgetsLayouts.render(mc, c, capture, mx, my);
                case "list" -> WidgetsLists.render(mc, c, capture, mx, my);
                default -> widget(mc, c).extractRenderState(capture, mx, my, 0.0F);
            }
            if (scissor != null) capture.disableScissor();
            return capture.draws;
        });
    }

    /** EditBox: its options, value and cursor moves, then focus and the caret clock. */
    static final class EditBoxCase {
        static AbstractWidget build(Minecraft mc, JsonObject c, int x, int y, int w, int h, Component msg) throws Exception {
            var box = new EditBox(mc.font, x, y, w, h, msg);
            box.setMaxLength(i(c, "maxLength", 32));
            box.setBordered(b(c, "bordered", true));
            box.setEditable(b(c, "editable", true));
            box.setCentered(b(c, "centered", false));
            box.setTextShadow(b(c, "shadow", true));
            box.setInvertHighlightedTextColor(b(c, "invert", true));
            if (c.has("textColor")) box.setTextColor(c.get("textColor").getAsInt());
            box.setValue(s(c, "value", ""));
            for (var op : c.getAsJsonArray("ops")) {
                var a = op.getAsJsonArray();
                if (a.get(0).getAsString().equals("cursor")) box.moveCursorTo(a.get(1).getAsInt(), a.get(2).getAsBoolean());
                else box.setHighlightPos(a.get(1).getAsInt());
            }
            if (c.has("suggestion")) box.setSuggestion(s(c, "suggestion", null));
            if (c.has("hint")) box.setHint(component(c.get("hint")));
            common(box, c);
            Stub.put(box, EditBox.class, "focusedTime", Util.getMillis() - i(c, "focusAgo", 0));
            return box;
        }
    }
}
