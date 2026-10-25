package acacia.parity.gui;

import com.google.gson.JsonParser;
import com.mojang.blaze3d.font.GlyphProvider;
import com.mojang.blaze3d.font.UnbakedGlyph;
import com.mojang.serialization.JsonOps;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GlyphSource;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import net.minecraft.client.gui.font.providers.GlyphProviderDefinition;
import net.minecraft.network.chat.FontDescription;
import net.minecraft.resources.Identifier;
import net.minecraft.util.RandomSource;

/**
 * GuiGame's metrics font with the galactic `minecraft:alt` font beside it (font/alt.json's bitmap
 * and the space include), for the enchanting table's option names.
 */
// TODO: drop once GuiGame's font is gui/java-parity's GuiFonts, which loads every font.
final class StationFont {
    private StationFont() {}

    private static Font font;

    static Font get() throws Exception {
        if (font != null) return font;
        var provider = (Font.Provider) Stub.get(GuiGame.font, Font.class, "provider");
        GlyphSource fallback = provider.glyphs(FontDescription.DEFAULT);
        List<GlyphProvider> alt = new ArrayList<>();
        for (String path : new String[] {"font/include/space.json", "font/alt.json"}) {
            try (var reader = new InputStreamReader(GuiGame.resources.open(Identifier.withDefaultNamespace(path)), StandardCharsets.UTF_8)) {
                for (var element : JsonParser.parseReader(reader).getAsJsonObject().getAsJsonArray("providers")) {
                    var conditional = GlyphProviderDefinition.Conditional.CODEC.parse(JsonOps.INSTANCE, element).getOrThrow();
                    conditional.definition().unpack().left().ifPresent(d -> {
                        try {
                            alt.add(d.load(GuiGame.resources));
                        } catch (Exception e) {
                            throw new RuntimeException(e);
                        }
                    });
                }
            }
        }
        GlyphSource galactic = new GlyphSource() {
            @Override public BakedGlyph getGlyph(int codepoint) {
                for (GlyphProvider p : alt) {
                    UnbakedGlyph glyph = p.getGlyph(codepoint);
                    if (glyph != null) return baked(glyph);
                }
                return fallback.getGlyph(codepoint);
            }
            @Override public BakedGlyph getRandomGlyph(RandomSource random, int width) { return getGlyph('?'); }

            private BakedGlyph baked(UnbakedGlyph glyph) {
                var info = glyph.info();
                return new BakedGlyph() {
                    @Override public com.mojang.blaze3d.font.GlyphInfo info() { return info; }
                    @Override public net.minecraft.client.gui.font.TextRenderable.Styled createGlyph(float x, float y, int color, int shadowColor,
                            net.minecraft.network.chat.Style style, float boldOffset, float shadowOffset) {
                        return null;
                    }
                };
            }
        };
        var altId = new FontDescription.Resource(Identifier.withDefaultNamespace("alt"));
        font = new Font(new Font.Provider() {
            @Override public GlyphSource glyphs(FontDescription description) { return altId.equals(description) ? galactic : fallback; }
            @Override public net.minecraft.client.gui.font.glyphs.EffectGlyph effect() { return provider.effect(); }
        });
        return font;
    }
}
