package acacia.parity;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.function.Consumer;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.font.TextRenderable;
import net.minecraft.client.gui.render.GuiRenderer;
import net.minecraft.client.gui.render.TextureSetup;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.client.renderer.state.gui.BlitRenderState;
import net.minecraft.client.renderer.state.gui.GlyphRenderState;
import net.minecraft.client.renderer.state.gui.GuiElementRenderState;
import net.minecraft.client.renderer.state.gui.GuiRenderState;
import net.minecraft.resources.Identifier;

/**
 * GUI draw calls without a window: a real GuiGraphicsExtractor over Client's Minecraft records
 * what a frame draws into a GuiRenderState, exactly as the game builds it before GuiRenderer
 * uploads anything. Window size and GUI scale come from Client.window. Blits name their texture
 * (gui sprites as `atlas#sprite`, UVs sprite-local in [0,1]); text keeps its styled runs, the
 * font's metrics are Fonts'. Horizon recorded by overriding GuiGraphics; reading the render state
 * the game already fills needs no override.
 */
public final class Gui {
    private Gui() {}

    /** One frame: `draw` extracts into a fresh GuiRenderState with the mouse at (mouseX, mouseY). */
    public static GuiRenderState capture(int mouseX, int mouseY, Consumer<GuiGraphicsExtractor> draw) throws Exception {
        var mc = Client.minecraft();
        var state = new GuiRenderState();
        var graphics = new GuiGraphicsExtractor(mc, state, mouseX, mouseY);
        draw.accept(graphics);
        graphics.extractDeferredElements(mouseX, mouseY, 0.0F);
        return state;
    }

    /**
     * The frame's records in the order GuiRenderer draws each kind, every record a Dump tree
     * tagged with `kind` (its class name, BlitRenderState...): elements (blits, fills, glyphs),
     * then text, items, and picture-in-picture states.
     */
    public static Map<String, Object> records(GuiRenderState state) {
        var elements = new ArrayList<Object>();
        state.forEachElement(e -> elements.add(tagged(e)), GuiRenderState.TraverseRange.ALL);
        var text = new ArrayList<Object>();
        state.forEachText(t -> text.add(tagged(t)));
        var items = new ArrayList<Object>();
        state.forEachItem(i -> items.add(tagged(i)));
        var pictures = new ArrayList<Object>();
        state.forEachPictureInPicture(p -> pictures.add(tagged(p)));
        return Out.map("elements", elements, "text", text, "items", items, "pictures", pictures);
    }

    /**
     * What GuiRenderer.prepare turns the state into, in its draw order: each item becomes a blit
     * of texture `parity:gui_item/<i>` (i indexes `items`, UVs [0,1]; oversized items are left
     * out), text becomes glyph states on its layer, elements are sorted with GuiRenderer's own
     * comparator, then walked before and after the blur. Each element gets `afterBlur`.
     * Picture-in-picture states are not blitted (see `pictures`). Gotcha: within one scissor and
     * pipeline the game orders textures by GpuTextureView identity hash, which is arbitrary in game
     * too; here it is the texture name's hash. Consumes the state: capture a fresh one per frame.
     */
    public static Map<String, Object> frame(GuiRenderState state) throws Exception {
        var records = records(state);
        int[] item = {0};
        state.forEachItem(i -> {
            int index = item[0]++;
            if (i.oversizedItemBounds() != null) return;
            var view = new Textures.NamedView(Identifier.fromNamespaceAndPath("parity", "gui_item/" + index));
            state.addBlitToCurrentLayer(new BlitRenderState(RenderPipelines.GUI_TEXTURED_PREMULTIPLIED_ALPHA,
                TextureSetup.singleTexture(view, null), i.pose(), i.x(), i.y(), i.x() + 16, i.y() + 16, 0.0F, 1.0F, 0.0F, 1.0F, -1,
                i.scissorArea(), null));
        });
        state.forEachText(text -> text.ensurePrepared().visit(new Font.GlyphVisitor() {
            @Override
            public void acceptRenderable(TextRenderable renderable) {
                state.addGlyphToCurrentLayer(new GlyphRenderState(text.pose, renderable, text.scissor));
            }
        }));
        Comparator<GuiElementRenderState> order = Shell.get(GuiRenderer.class, "ELEMENT_SORT_COMPARATOR");
        state.sortElements(order);
        var draws = new ArrayList<Object>();
        for (var range : new GuiRenderState.TraverseRange[]{GuiRenderState.TraverseRange.BEFORE_BLUR, GuiRenderState.TraverseRange.AFTER_BLUR}) {
            state.forEachElement(e -> {
                @SuppressWarnings("unchecked") var tree = (Map<String, Object>) tagged(e);
                tree.put("afterBlur", range == GuiRenderState.TraverseRange.AFTER_BLUR);
                draws.add(tree);
            }, range);
        }
        var out = new java.util.TreeMap<>(records);
        out.put("draws", draws);
        return out;
    }

    /** capture then frame. */
    public static Map<String, Object> frame(int mouseX, int mouseY, Consumer<GuiGraphicsExtractor> draw) throws Exception {
        return frame(capture(mouseX, mouseY, draw));
    }

    /** capture then records. */
    public static Map<String, Object> record(int mouseX, int mouseY, Consumer<GuiGraphicsExtractor> draw) throws Exception {
        return records(capture(mouseX, mouseY, draw));
    }

    private static Object tagged(Object record) {
        var tree = Dump.of(record);
        if (!(tree instanceof Map<?, ?> fields)) return List.of(record.getClass().getSimpleName(), tree);
        @SuppressWarnings("unchecked") var out = new java.util.TreeMap<>((Map<String, Object>) fields);
        out.put("kind", record.getClass().getSimpleName());
        return out;
    }
}
