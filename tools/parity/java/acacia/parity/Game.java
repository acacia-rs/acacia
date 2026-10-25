package acacia.parity;

import java.util.List;
import java.util.Optional;
import net.minecraft.SharedConstants;
import net.minecraft.client.ClientBootstrap;
import net.minecraft.client.color.block.BlockColors;
import net.minecraft.client.resources.LegacyStuffWrapper;
import net.minecraft.core.HolderLookup;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.packs.PackLocationInfo;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.VanillaPackResourcesBuilder;
import net.minecraft.server.packs.repository.BuiltInPackSource;
import net.minecraft.server.packs.repository.PackSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.world.level.DryFoliageColor;
import net.minecraft.world.level.FoliageColor;
import net.minecraft.world.level.GrassColor;

/**
 * The headless game every oracle starts from: version, registries and Bootstrap, the client jar's
 * vanilla resource pack as a ResourceManager (no window, no GPU, no asset index), the colour maps
 * the client's reload listeners would install, and vanilla's world registries.
 */
public final class Game {
    private static ResourceManager resources;
    private static HolderLookup.Provider world;
    private static BlockColors blockColors;

    private Game() {}

    /** Idempotent. */
    public static synchronized void init() throws Exception {
        if (resources != null) return;
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        ClientBootstrap.bootstrap();
        var info = new PackLocationInfo(BuiltInPackSource.VANILLA_ID, Component.literal("vanilla"), PackSource.BUILT_IN,
            Optional.of(BuiltInPackSource.CORE_PACK_INFO));
        var pack = new VanillaPackResourcesBuilder().exposeNamespace("minecraft", "realms").pushJarResources().build(info);
        resources = new MultiPackResourceManager(PackType.CLIENT_RESOURCES, List.of(pack.fullResources()));
        GrassColor.init(LegacyStuffWrapper.getPixels(resources, Identifier.withDefaultNamespace("textures/colormap/grass.png")));
        FoliageColor.init(LegacyStuffWrapper.getPixels(resources, Identifier.withDefaultNamespace("textures/colormap/foliage.png")));
        DryFoliageColor.init(LegacyStuffWrapper.getPixels(resources, Identifier.withDefaultNamespace("textures/colormap/dry_foliage.png")));
    }

    /** The client resources: what the vanilla pack of a fresh client holds. */
    public static ResourceManager resources() {
        return resources;
    }

    /** Vanilla's built-in world registries (biomes, dimension types...) with their tags unbound. */
    public static synchronized HolderLookup.Provider world() {
        if (world == null) world = VanillaRegistries.createWorldLookup();
        return world;
    }

    public static synchronized BlockColors blockColors() {
        if (blockColors == null) blockColors = BlockColors.createDefault();
        return blockColors;
    }
}
