package acacia.parity.gui;

import com.google.common.collect.Lists;
import com.mojang.blaze3d.font.GlyphBitmap;
import com.mojang.blaze3d.font.GlyphInfo;
import com.mojang.blaze3d.font.GlyphProvider;
import java.lang.reflect.Method;
import java.nio.IntBuffer;
import java.util.Arrays;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.Executor;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GlyphSource;
import net.minecraft.client.gui.font.AllMissingGlyphProvider;
import net.minecraft.client.gui.font.FontManager;
import net.minecraft.client.gui.font.FontOption;
import net.minecraft.client.gui.font.FontSet;
import net.minecraft.client.gui.font.GlyphStitcher;
import net.minecraft.client.gui.font.glyphs.BakedSheetGlyph;
import net.minecraft.client.gui.font.glyphs.EffectGlyph;
import net.minecraft.client.gui.font.glyphs.SpecialGlyphs;
import net.minecraft.client.gui.font.providers.UnihexProvider;
import net.minecraft.network.chat.FontDescription;
import net.minecraft.resources.Identifier;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.world.level.levelgen.LegacyRandomSource;

/**
 * The game's fonts as FontManager loads them, without a GPU: FontManager.prepare resolves every
 * font definition's providers (Unifont included when the resources have the asset index), and
 * vanilla FontSets select and bake glyphs over a stitcher that, instead of packing a font atlas,
 * bakes each glyph against its source: texels of its sheet (UVs nudged 0.01 in as FontTexture
 * nudges them), one Unifont glyph, or a special glyph. `texture(glyph)` names that source.
 */
public final class GuiFonts {
    public final Map<Identifier, FontSet> sets = new HashMap<>();
    public final FontSet missing;
    private final Map<BakedSheetGlyph, String> textures = new IdentityHashMap<>();
    private final Map<BakedSheetGlyph, Boolean> colored = new IdentityHashMap<>();
    private final Map<Object, Integer> unihex = new IdentityHashMap<>();

    @SuppressWarnings("unchecked")
    public GuiFonts(ResourceManager resources, Set<FontOption> options) throws Exception {
        FontManager manager = Stub.alloc(FontManager.class);
        Method prepare = Stub.method(FontManager.class, "prepare", ResourceManager.class, Executor.class);
        Object preparation = ((CompletableFuture<?>) prepare.invoke(manager, resources, (Executor) Runnable::run)).join();
        Method fontSets = preparation.getClass().getDeclaredMethod("fontSets");
        fontSets.setAccessible(true);
        var resolved = (Map<Identifier, List<GlyphProvider.Conditional>>) fontSets.invoke(preparation);
        for (var list : resolved.values()) {
            for (var c : list) {
                if (c.provider() instanceof UnihexProvider p) {
                    p.getSupportedGlyphs().forEach(cp -> unihex.put(p.getGlyph(cp), cp));
                }
            }
        }
        for (var e : resolved.entrySet()) {
            FontSet set = new FontSet(new Stitcher(e.getKey()));
            // FontManager.apply: the resolved lists are reversed.
            set.reload(Lists.reverse(e.getValue()), options);
            sets.put(e.getKey(), set);
        }
        missing = new FontSet(new Stitcher(FontManager.MISSING_FONT));
        missing.reload(List.of(new GlyphProvider.Conditional(new AllMissingGlyphProvider(), FontOption.Filter.ALWAYS_PASS)), options);
    }

    public FontSet set(FontDescription d) {
        return d instanceof FontDescription.Resource r ? sets.getOrDefault(r.id(), missing) : missing;
    }

    /** FontManager.CachedFontProvider's font (`createFont`, or `createFontFilterFishy`), its random seeded. */
    public Font font(boolean filterFishy, long seed) throws Exception {
        Font font = new Font(new Font.Provider() {
            @Override public GlyphSource glyphs(FontDescription d) { return set(d).source(filterFishy); }
            @Override public EffectGlyph effect() { return sets.get(FontDescription.DEFAULT.id()).whiteGlyph(); }
        });
        seed(font, seed);
        return font;
    }

    public static void seed(Font font, long seed) throws Exception {
        Stub.put(font, Font.class, "random", new LegacyRandomSource(seed));
    }

    /** The glyph's source: `minecraft:textures/font/x.png`, `unihex:<cp hex>:<texels hash>`, `missing`, `white`. */
    public String texture(BakedSheetGlyph glyph) {
        return textures.get(glyph);
    }

    public boolean colored(BakedSheetGlyph glyph) {
        return colored.get(glyph);
    }

    private static Object record(Object o, String accessor) throws Exception {
        Method m = o.getClass().getDeclaredMethod(accessor);
        m.setAccessible(true);
        return m.invoke(o);
    }

    private final class Stitcher extends GlyphStitcher {
        Stitcher(Identifier id) {
            super(null, id);
        }

        @Override public void reset() {}

        @Override public BakedSheetGlyph stitch(GlyphInfo info, GlyphBitmap bitmap) {
            try {
                Object outer = Stub.get(bitmap, bitmap.getClass(), "this$0");
                int x = 0;
                int y = 0;
                String key;
                if (outer instanceof SpecialGlyphs s) {
                    key = s == SpecialGlyphs.MISSING ? "missing" : "white";
                } else if (outer.getClass().getName().endsWith("BitmapProvider$Glyph")) {
                    x = (int) record(outer, "offsetX");
                    y = (int) record(outer, "offsetY");
                    Object holder = record(outer, "imageData");
                    key = Stub.get(holder, holder.getClass(), "identifier").toString();
                } else {
                    int w = bitmap.getPixelWidth();
                    IntBuffer texels = IntBuffer.allocate(w * 16);
                    Method unpack = Stub.method(UnihexProvider.class, "unpackBitsToBytes", IntBuffer.class,
                        Class.forName("net.minecraft.client.gui.font.providers.UnihexProvider$LineData"), int.class, int.class);
                    unpack.invoke(null, texels, record(outer, "contents"), record(outer, "left"), record(outer, "right"));
                    key = "unihex:" + Integer.toHexString(unihex.get(outer)) + ":" + Arrays.hashCode(texels.array());
                }
                int w = bitmap.getPixelWidth();
                int h = bitmap.getPixelHeight();
                BakedSheetGlyph glyph = new BakedSheetGlyph(info, null, null, x + 0.01F, x - 0.01F + w, y + 0.01F, y - 0.01F + h,
                    bitmap.getLeft(), bitmap.getRight(), bitmap.getTop(), bitmap.getBottom());
                textures.put(glyph, key);
                colored.put(glyph, bitmap.isColored());
                return glyph;
            } catch (Exception e) {
                throw new IllegalStateException(e);
            }
        }
    }
}
