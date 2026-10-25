package acacia.parity;

import java.util.List;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.component.DataComponentInitializers;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.ReloadableServerRegistries;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagLoader;

/**
 * The vanilla data pack loaded into registries as a server loads it: world registries (biomes,
 * dimension types, damage types...) with tags bound, loot tables, default item components.
 * What a client receives from a vanilla server. Adapted from Horizon's ParityData (MIT).
 */
public final class Data {
    private static Data instance;

    public final RegistryAccess.Frozen access;
    public final HolderLookup.Provider lookup;
    public final ReloadableServerRegistries.Holder loot;

    private Data() {
        var repo = ServerPacksSource.createVanillaTrustedRepository();
        repo.reload();
        repo.setSelected(List.of("vanilla"));
        var resources = new MultiPackResourceManager(PackType.SERVER_DATA, repo.openAllSelected());
        var layers = RegistryLayer.createRegistryAccess();
        var staticTags = TagLoader.loadTagsForExistingRegistries(resources, layers.getLayer(RegistryLayer.STATIC));
        var context = TagLoader.buildUpdatedLookups(layers.getAccessForLoading(RegistryLayer.WORLD), staticTags);
        RegistryAccess.Frozen world = RegistryDataLoader.load(resources, context, RegistryDataLoader.WORLD_REGISTRIES, Runnable::run).join();
        var worldLayers = layers.replaceFrom(RegistryLayer.WORLD, world);
        var loaded = ReloadableServerRegistries.reload(worldLayers, staticTags, resources, Runnable::run).join();
        lookup = loaded.lookupWithUpdatedTags();
        loot = new ReloadableServerRegistries.Holder(lookup);
        var components = BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(lookup);
        staticTags.forEach(Registry.PendingTags::apply);
        components.forEach(DataComponentInitializers.PendingComponents::apply);
        access = loaded.layers().compositeAccess();
    }

    public static synchronized Data get() throws Exception {
        if (instance == null) {
            Game.init();
            instance = new Data();
        }
        return instance;
    }
}
