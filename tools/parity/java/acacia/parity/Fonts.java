package acacia.parity;

import com.google.gson.JsonParser;
import com.mojang.blaze3d.font.GlyphBitmap;
import com.mojang.blaze3d.font.GlyphInfo;
import com.mojang.blaze3d.font.GlyphProvider;
import com.mojang.serialization.JsonOps;
import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.EnumSet;
import java.util.List;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GlyphSource;
import net.minecraft.client.gui.font.FontOption;
import net.minecraft.client.gui.font.FontSet;
import net.minecraft.client.gui.font.GlyphRenderTypes;
import net.minecraft.client.gui.font.GlyphStitcher;
import net.minecraft.client.gui.font.glyphs.BakedSheetGlyph;
import net.minecraft.client.gui.font.glyphs.EffectGlyph;
import net.minecraft.client.gui.font.providers.GlyphProviderDefinition;
import net.minecraft.network.chat.FontDescription;
import net.minecraft.resources.Identifier;

/**
 * The default font without a GPU: a real FontSet over the jar's font/default.json references
 * (space, default; filters merged as FontManager merges them), so advances, bold/shadow offsets,
 * glyph bounds, the missing and white glyphs and random (obfuscated) glyph picks are the game's.
 * Only the stitcher is replaced: glyph bitmaps are measured, never uploaded, and every glyph
 * samples Textures.NamedView `minecraft:font/glyphs` with UVs [0,1]. The unifont include is in the
 * asset index, not the jar: a codepoint only it covers comes out as the missing glyph.
 * Adapted from Horizon's ParityFont (MIT).
 */
public final class Fonts {
    private static final Identifier GLYPHS = Identifier.withDefaultNamespace("font/glyphs");
    private static Font font;
    private static FontSet fontSet;

    private Fonts() {}

    /** Measures, never uploads. Allocated, never constructed. */
    static final class MeasuringStitcher extends GlyphStitcher {
        private MeasuringStitcher() {
            super(null, null);
        }

        @Override
        public BakedSheetGlyph stitch(GlyphInfo info, GlyphBitmap bitmap) {
            var types = bitmap.isColored() ? GlyphRenderTypes.createForColorTexture(GLYPHS) : GlyphRenderTypes.createForGrayscaleTexture(GLYPHS);
            return new BakedSheetGlyph(info, types, new Textures.NamedView(GLYPHS), 0.0F, 1.0F, 0.0F, 1.0F,
                bitmap.getLeft(), bitmap.getRight(), bitmap.getTop(), bitmap.getBottom());
        }

        @Override
        public void reset() {}
    }

    public static synchronized Font font() throws Exception {
        if (font != null) return font;
        Game.init();
        var providers = new ArrayList<GlyphProvider.Conditional>();
        for (var reference : definitions(Identifier.withDefaultNamespace("font/default.json"))) {
            var target = reference.definition().unpack().right().orElseThrow().id();
            var file = target.withPath(p -> "font/" + p + ".json");
            if (Game.resources().getResource(file).isEmpty()) continue;
            for (var inner : definitions(file)) {
                var provider = inner.definition().unpack().left().orElseThrow().load(Game.resources());
                providers.add(new GlyphProvider.Conditional(provider, reference.filter().merge(inner.filter())));
            }
        }
        fontSet = new FontSet(Shell.alloc(MeasuringStitcher.class));
        fontSet.reload(providers, EnumSet.noneOf(FontOption.class));
        font = new Font(new Font.Provider() {
            @Override
            public GlyphSource glyphs(FontDescription description) {
                return fontSet.source(false);
            }

            @Override
            public EffectGlyph effect() {
                return fontSet.whiteGlyph();
            }
        });
        return font;
    }

    private static List<GlyphProviderDefinition.Conditional> definitions(Identifier file) throws IOException {
        var out = new ArrayList<GlyphProviderDefinition.Conditional>();
        try (var reader = new InputStreamReader(Game.resources().open(file), StandardCharsets.UTF_8)) {
            for (var element : JsonParser.parseReader(reader).getAsJsonObject().getAsJsonArray("providers")) {
                out.add(GlyphProviderDefinition.Conditional.CODEC.parse(JsonOps.INSTANCE, element).getOrThrow());
            }
        }
        return out;
    }
}
