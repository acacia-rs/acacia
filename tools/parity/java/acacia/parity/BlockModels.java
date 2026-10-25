package acacia.parity;

import java.io.Reader;
import java.util.HashMap;
import java.util.Map;
import net.minecraft.client.model.geom.EntityModelSet;
import net.minecraft.client.renderer.block.dispatch.BlockStateModel;
import net.minecraft.client.renderer.texture.TextureAtlas;
import net.minecraft.client.resources.model.BlockStateModelLoader;
import net.minecraft.client.resources.model.ModelBakery;
import net.minecraft.client.resources.model.ModelDiscovery;
import net.minecraft.client.resources.model.UnbakedModel;
import net.minecraft.client.resources.model.cuboid.CuboidModel;
import net.minecraft.client.resources.model.cuboid.ItemModelGenerator;
import net.minecraft.client.resources.model.cuboid.MissingCuboidModel;
import net.minecraft.client.resources.model.sprite.MaterialBaker;
import net.minecraft.client.renderer.texture.SpriteLoader;
import net.minecraft.data.AtlasIds;
import net.minecraft.resources.FileToIdConverter;
import net.minecraft.resources.Identifier;
import net.minecraft.world.level.block.state.BlockState;

/**
 * Every block state's baked model, by the client's own loaders and ModelBakery as ModelManager's
 * reload runs them (blockstates, models, discovery, bake), over the block and item atlases of
 * UnitAtlas. Item models are not baked.
 */
public final class BlockModels {
    private static final FileToIdConverter MODELS = FileToIdConverter.json("models");
    public final SpriteLoader.Preparations blockAtlas, itemAtlas;
    public final ModelBakery.BakingResult baked;

    private BlockModels() throws Exception {
        var resources = Game.resources();
        Map<Identifier, UnbakedModel> models = new HashMap<>();
        for (var entry : MODELS.listMatchingResources(resources).entrySet()) {
            try (Reader reader = entry.getValue().openAsReader()) {
                models.put(MODELS.fileToId(entry.getKey()), CuboidModel.fromStream(reader));
            }
        }
        var states = BlockStateModelLoader.loadBlockStates(resources, Runnable::run).join();
        var discovery = new ModelDiscovery(models, MissingCuboidModel.missingModel());
        discovery.addSpecialModel(ItemModelGenerator.GENERATED_ITEM_MODEL_ID, new ItemModelGenerator());
        states.models().values().forEach(discovery::addRoot);
        var resolved = discovery.resolve();
        blockAtlas = UnitAtlas.load(AtlasIds.BLOCKS, TextureAtlas.LOCATION_BLOCKS);
        itemAtlas = UnitAtlas.load(AtlasIds.ITEMS, TextureAtlas.LOCATION_ITEMS);
        var bakery = new ModelBakery(EntityModelSet.vanilla(), null, null, states.models(), Map.of(), resolved, discovery.missingModel());
        baked = bakery.bakeModels(new MaterialBaker(blockAtlas, itemAtlas), Runnable::run).join();
    }

    private static BlockModels instance;

    public static synchronized BlockModels get() throws Exception {
        if (instance == null) {
            Game.init();
            instance = new BlockModels();
        }
        return instance;
    }

    /** The state's model, or null where the resources define none (the client draws the missing model). */
    public BlockStateModel model(BlockState state) {
        return baked.blockStateModels().get(state);
    }
}
