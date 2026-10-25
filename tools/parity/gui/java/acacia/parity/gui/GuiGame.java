package acacia.parity.gui;

import com.google.gson.JsonParser;
import com.mojang.blaze3d.font.GlyphInfo;
import com.mojang.blaze3d.font.GlyphProvider;
import com.mojang.blaze3d.font.UnbakedGlyph;
import com.mojang.serialization.JsonOps;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GlyphSource;
import net.minecraft.client.gui.font.TextRenderable;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import net.minecraft.client.gui.font.providers.GlyphProviderDefinition;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.chat.FontDescription;
import net.minecraft.network.chat.Style;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.ReloadableServerRegistries;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.tags.TagLoader;
import net.minecraft.util.RandomSource;

/**
 * The headless game the GUI oracles share: Bootstrap and the vanilla data pack's registries (item
 * components bound, as a server's), the client jar's resources, and the default font's metrics
 * from vanilla's own glyph providers (no atlas, nothing stitched). Adapted from Horizon's
 * ParityData and ParityFont (MIT, (c) Ludovic Marie).
 */
public final class GuiGame {
    public static HolderLookup.Provider lookup;
    public static ResourceManager resources;
    public static Font font;

    private GuiGame() {}

    public static synchronized void init() throws Exception {
        if (lookup != null) return;
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var repo = ServerPacksSource.createVanillaTrustedRepository();
        repo.reload();
        repo.setSelected(List.of("vanilla"));
        var data = new MultiPackResourceManager(PackType.SERVER_DATA, repo.openAllSelected());
        var layers = RegistryLayer.createRegistryAccess();
        var staticTags = TagLoader.loadTagsForExistingRegistries(data, layers.getLayer(RegistryLayer.STATIC));
        var context = TagLoader.buildUpdatedLookups(layers.getAccessForLoading(RegistryLayer.WORLD), staticTags);
        var world = RegistryDataLoader.load(data, context, RegistryDataLoader.WORLD_REGISTRIES, Runnable::run).join();
        var loaded = ReloadableServerRegistries.reload(layers.replaceFrom(RegistryLayer.WORLD, world), staticTags, data, Runnable::run).join();
        lookup = loaded.lookupWithUpdatedTags();
        var components = BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(lookup);
        staticTags.forEach(Registry.PendingTags::apply);
        components.forEach(net.minecraft.core.component.DataComponentInitializers.PendingComponents::apply);
        var method = net.minecraft.client.resources.ClientPackSource.class.getDeclaredMethod("createVanillaPackSource", Path.class);
        method.setAccessible(true);
        resources = ((net.minecraft.server.packs.VanillaPackResources) method.invoke(null, Path.of("/nonexistent-assets"))).asResourceManager();
        font = defaultFont(resources);
    }

    /** `font/include/space.json` and `default.json`'s providers: metrics only. */
    static Font defaultFont(ResourceManager resources) throws Exception {
        List<GlyphProvider> providers = new ArrayList<>();
        for (String include : new String[] {"space", "default"}) {
            var id = Identifier.withDefaultNamespace("font/include/" + include + ".json");
            try (var reader = new InputStreamReader(resources.open(id), StandardCharsets.UTF_8)) {
                for (var element : JsonParser.parseReader(reader).getAsJsonObject().getAsJsonArray("providers")) {
                    var conditional = GlyphProviderDefinition.Conditional.CODEC.parse(JsonOps.INSTANCE, element).getOrThrow();
                    providers.add(conditional.definition().unpack().left().orElseThrow().load(resources));
                }
            }
        }
        GlyphSource source = new GlyphSource() {
            @Override public BakedGlyph getGlyph(int codepoint) {
                for (GlyphProvider provider : providers) {
                    UnbakedGlyph glyph = provider.getGlyph(codepoint);
                    if (glyph != null) return baked(glyph.info());
                }
                return baked(GlyphInfo.simple(6.0F));
            }
            @Override public BakedGlyph getRandomGlyph(RandomSource random, int width) { return getGlyph('?'); }
        };
        return new Font(new Font.Provider() {
            @Override public GlyphSource glyphs(FontDescription font) { return source; }
            @Override public net.minecraft.client.gui.font.glyphs.EffectGlyph effect() { return null; }
        });
    }

    private static BakedGlyph baked(GlyphInfo info) {
        return new BakedGlyph() {
            @Override public GlyphInfo info() { return info; }
            @Override public TextRenderable.Styled createGlyph(float x, float y, int color, int shadowColor, Style style, float boldOffset, float shadowOffset) {
                return null;
            }
        };
    }
}
