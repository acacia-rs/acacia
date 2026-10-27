package acacia.parity.gui;

import com.google.gson.JsonObject;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.ObjectSelectionList;
import net.minecraft.network.chat.Component;

/**
 * The `list` kind: an ObjectSelectionList whose entries print their label at their content
 * corner (white with a shadow, yellow when hovered), sized, scrolled, selected and focused as the
 * case says.
 */
final class WidgetsLists {
    private WidgetsLists() {}

    static final class Row extends ObjectSelectionList.Entry<Row> {
        final Component label;
        Row(Component label) { this.label = label; }
        @Override public void extractContent(GuiGraphicsExtractor g, int mx, int my, boolean hovered, float a) {
            g.text(Minecraft.getInstance().font, label, getContentX(), getContentY(), hovered ? -256 : -1, true);
        }
        @Override public Component getNarration() { return label; }
    }

    static final class TestList extends ObjectSelectionList<Row> {
        final int rowWidth;
        TestList(Minecraft mc, int w, int h, int y, int itemHeight, int rowWidth) {
            super(mc, w, h, y, itemHeight);
            this.rowWidth = rowWidth;
        }
        @Override public int getRowWidth() { return rowWidth; }
        void add(Row e, int height) { addEntry(e, height); }
    }

    static void render(Minecraft mc, JsonObject c, GuiCapture capture, int mx, int my) {
        var r = c.getAsJsonArray("rect");
        var list = new TestList(mc, r.get(2).getAsInt(), r.get(3).getAsInt(), r.get(1).getAsInt(), 20, c.get("rowWidth").getAsInt());
        var heights = c.getAsJsonArray("rows");
        var rows = new java.util.ArrayList<Row>();
        for (int n = 0; n < heights.size(); n++) {
            var e = new Row(Component.literal("Row " + n));
            rows.add(e);
            list.add(e, heights.get(n).getAsInt());
        }
        list.updateSizeAndPosition(r.get(2).getAsInt(), r.get(3).getAsInt(), r.get(0).getAsInt(), r.get(1).getAsInt());
        list.setScrollAmount(c.get("scroll").getAsDouble());
        int selected = ScreenGame.i(c, "selected", -1);
        if (selected >= 0) {
            if (ScreenGame.b(c, "focused", false)) list.setFocused(rows.get(selected));
            else list.setSelected(rows.get(selected));
        }
        list.extractRenderState(capture, mx, my, 0.0F);
    }
}
