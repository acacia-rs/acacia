package acacia.parity;

import java.util.Arrays;
import java.util.List;
import java.util.Set;
import net.minecraft.client.renderer.block.FluidRenderer;
import net.minecraft.client.renderer.block.FluidStateModelSet;
import net.minecraft.client.renderer.texture.SpriteContents;
import net.minecraft.client.renderer.texture.SpriteLoader;
import net.minecraft.client.renderer.texture.TextureAtlas;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.resources.model.ModelDebugName;
import net.minecraft.client.resources.model.sprite.Material;
import net.minecraft.client.resources.model.sprite.MaterialBaker;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.AtlasIds;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.VanillaPackResourcesBuilder;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagKey;
import net.minecraft.tags.TagLoader;
import net.minecraft.world.level.block.Block;

/**
 * The client's `FluidRenderer`, for the `fluids` and `world_light` harnesses. Fluid sprites sit
 * side by side in one strip with a gap after each, so a vertex's u says which sprite it samples
 * (after Horizon's VanillaSectionMeshOracle).
 */
public final class FluidRendering {
    public static final String[] SPRITES = {"minecraft:block/water_still", "minecraft:block/water_flow",
        "minecraft:block/water_overlay", "minecraft:block/lava_still", "minecraft:block/lava_flow"};
    public static final int STRIP = 2 * SPRITES.length;

    private FluidRendering() {}

    private static final class Sprite extends TextureAtlasSprite {
        Sprite(SpriteContents contents, int slot) {
            super(TextureAtlas.LOCATION_BLOCKS, contents, contents.width() * STRIP, contents.height(), contents.width() * 2 * slot, 0, 0);
        }
    }

    /** The client's own sprites (their transparency picks the fluid's layer), each moved into its strip slot. */
    private static final class Materials extends MaterialBaker {
        final SpriteLoader.Preparations atlas;

        Materials(SpriteLoader.Preparations atlas) {
            super(atlas, atlas);
            this.atlas = atlas;
        }

        @Override
        public Material.Baked get(Material material, ModelDebugName name) {
            int slot = Arrays.asList(SPRITES).indexOf(material.sprite().toString());
            var sprite = atlas.getSprite(material.sprite());
            if (slot < 0 || sprite == null) throw new AssertionError("unexpected fluid sprite " + material.sprite());
            return new Material.Baked(new Sprite(sprite.contents(), slot), material.forceTranslucent());
        }
    }

    /** Binds the jar's block tags (FlowingFluid.getFlow reads #blocks_fluid_flow). */
    private static void bindBlockTags() {
        var info = new net.minecraft.server.packs.PackLocationInfo(net.minecraft.server.packs.repository.BuiltInPackSource.VANILLA_ID,
            net.minecraft.network.chat.Component.literal("vanilla"), net.minecraft.server.packs.repository.PackSource.BUILT_IN,
            java.util.Optional.of(net.minecraft.server.packs.repository.BuiltInPackSource.CORE_PACK_INFO));
        var pack = new VanillaPackResourcesBuilder().exposeNamespace("minecraft").pushJarResources().build(info);
        var data = new MultiPackResourceManager(PackType.SERVER_DATA, List.of(pack.fullResources()));
        @SuppressWarnings("unchecked")
        var lookup = (TagLoader.ElementLookup<Holder<Block>>) TagLoader.ElementLookup.fromFrozenRegistry(BuiltInRegistries.BLOCK);
        var loader = new TagLoader<>(lookup, Registries.tagsDirPath(Registries.BLOCK));
        var tags = new java.util.HashMap<TagKey<Block>, List<Holder<Block>>>();
        loader.build(loader.load(data)).forEach((id, values) -> tags.put(TagKey.create(Registries.BLOCK, id), List.copyOf(values)));
        if (tags.isEmpty()) throw new AssertionError("no block tags in the jar");
        BuiltInRegistries.BLOCK.prepareTagReload(new TagLoader.LoadResult<>(Registries.BLOCK, tags)).apply();
    }

    /** The renderer over the strip sprites, with the jar's block tags bound. */
    public static FluidRenderer renderer() throws Exception {
        Game.init();
        bindBlockTags();
        var models = FluidStateModelSet.bake(new Materials(UnitAtlas.load(AtlasIds.BLOCKS, TextureAtlas.LOCATION_BLOCKS, Set.of(), false)));
        return new FluidRenderer(new FluidStateModelSet(models, models.values().iterator().next()));
    }
}
