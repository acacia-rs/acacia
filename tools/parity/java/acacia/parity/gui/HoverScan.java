package acacia.parity.gui;

import java.util.function.Consumer;
import net.minecraft.client.gui.ActiveTextCollector;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.GuiGraphicsExtractor.HoveredTextEffects;
import net.minecraft.client.gui.navigation.ScreenRectangle;
import net.minecraft.client.renderer.state.gui.GuiTextRenderState;
import net.minecraft.network.chat.Style;
import net.minecraft.util.FormattedCharSequence;
import org.joml.Matrix3x2fc;

/**
 * RenderingTextCollector's style scan for GuiCapture's text collectors (on with `scanHover`): the
 * text's styles under (hoverX, hoverY), the hovered one kept for the deferred tooltip, every hit
 * to the collector's consumer.
 */
record HoverScan(FormattedCharSequence text, Matrix3x2fc pose, int x, int y, int color, ScreenRectangle scissor) {
    void run(GuiCapture g, HoveredTextEffects effects, Consumer<Style> consumer) {
        if (!g.scanHover || (!effects.allowTooltip && !effects.allowCursorChanges && consumer == null)) return;
        var state = new GuiTextRenderState(g.font, text, pose, x, y, color, 0, true, true, scissor);
        ActiveTextCollector.findElementUnderCursor(state, g.hoverX, g.hoverY, style -> {
            if (effects.allowTooltip && style.getHoverEvent() != null) {
                try { Stub.put(g, GuiGraphicsExtractor.class, "hoveredTextStyle", style); } catch (Exception e) { throw new RuntimeException(e); }
            }
            if (consumer != null) consumer.accept(style);
        });
    }
}
