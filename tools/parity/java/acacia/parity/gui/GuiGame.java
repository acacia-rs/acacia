package acacia.parity.gui;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Set;
import net.minecraft.SharedConstants;
import net.minecraft.client.gui.Font;
import net.minecraft.client.resources.IndexedAssetSource;
import net.minecraft.client.resources.language.ClientLanguage;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.locale.Language;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.ReloadableServerRegistries;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.tags.TagLoader;

/**
 * The headless game the GUI oracles share: Bootstrap and the vanilla data pack's registries (item
 * components bound, as a server's), the client's resources (the jar's, under the asset index at
 * GUI_ASSETS_ROOT, default ~/gui-assets, which tools/parity/harnesses/font/fetch.py fills with the
 * font objects), ClientLanguage (en_us) as Language, and the client's default Font with every
 * font vanilla loads (GuiFonts), its random seeded with 0. Adapted from Horizon's ParityData and
 * ParityFont (MIT, (c) Ludovic Marie).
 */
public final class GuiGame {
    /** 26.3's assetIndex.id. */
    public static final String ASSET_INDEX = "34";
    public static HolderLookup.Provider lookup;
    /** The same registries as a RegistryAccess (a level's). */
    public static net.minecraft.core.RegistryAccess access;
    public static ResourceManager resources;
    public static GuiFonts fonts;
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
        access = loaded.layers().compositeAccess();
        var components = BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(lookup);
        staticTags.forEach(Registry.PendingTags::apply);
        components.forEach(net.minecraft.core.component.DataComponentInitializers.PendingComponents::apply);
        var method = net.minecraft.client.resources.ClientPackSource.class.getDeclaredMethod("createVanillaPackSource", Path.class);
        method.setAccessible(true);
        resources = ((net.minecraft.server.packs.VanillaPackResources) method.invoke(null, assetIndex())).asResourceManager();
        Language.inject(ClientLanguage.loadFrom(resources, List.of("en_us"), false));
        fonts = new GuiFonts(resources, Set.of());
        font = fonts.font(false, 0L);
    }

    /** The launcher's virtual assets root over the asset index, as Main builds it. */
    static Path assetIndex() {
        String root = System.getenv().getOrDefault("GUI_ASSETS_ROOT", System.getProperty("user.home") + "/gui-assets");
        if (!Files.isRegularFile(Path.of(root, "indexes", ASSET_INDEX + ".json"))) {
            throw new IllegalStateException("no asset index under " + root + ": run tools/parity/harnesses/font/fetch.py");
        }
        return IndexedAssetSource.createIndexFs(Path.of(root), ASSET_INDEX);
    }
}
