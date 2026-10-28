package acacia.parity;

import java.io.Reader;
import java.util.HashMap;
import java.util.Map;
import java.util.function.Function;
import net.minecraft.client.model.geom.EntityModelSet;
import net.minecraft.client.renderer.block.BuiltInBlockModels;
import net.minecraft.client.renderer.block.FluidStateModelSet;
import net.minecraft.client.renderer.block.LoadedBlockModels;
import net.minecraft.client.renderer.block.dispatch.BlockStateModel;
import net.minecraft.client.resources.model.BlockStateModelLoader;
import net.minecraft.client.resources.model.ClientItemInfoLoader;
import net.minecraft.client.resources.model.ModelBakery;
import net.minecraft.client.resources.model.ModelDiscovery;
import net.minecraft.client.resources.model.ModelManager;
import net.minecraft.client.resources.model.UnbakedModel;
import net.minecraft.client.resources.model.cuboid.CuboidModel;
import net.minecraft.client.resources.model.cuboid.ItemModelGenerator;
import net.minecraft.client.resources.model.cuboid.MissingCuboidModel;
import net.minecraft.client.resources.model.sprite.MaterialBaker;
import net.minecraft.data.AtlasIds;
import net.minecraft.resources.FileToIdConverter;
import net.minecraft.resources.Identifier;
import net.minecraft.world.level.block.state.BlockState;

/**
 * ModelManager's reload without a GPU: blockstates, models and client items by the client's own
 * loaders, discovery, ModelBakery over Atlases (block state, item and block models, fluids), applied
 * to a real ModelManager. The player skin cache is absent (player heads bake without skins).
 */
public final class Models {
    private static final FileToIdConverter MODELS = FileToIdConverter.json("models");
    private static Models instance;

    public final ModelManager manager;
    public final ModelBakery.BakingResult baked;
    public final EntityModelSet entityModels = EntityModelSet.vanilla();

    private Models() throws Exception {
        var resources = Game.resources();
        var atlases = Atlases.manager();
        Map<Identifier, UnbakedModel> models = new HashMap<>();
        for (var entry : MODELS.listMatchingResources(resources).entrySet()) {
            try (Reader reader = entry.getValue().openAsReader()) {
                models.put(MODELS.fileToId(entry.getKey()), CuboidModel.fromStream(reader));
            }
        }
        var states = BlockStateModelLoader.loadBlockStates(resources, Runnable::run).join();
        var items = ClientItemInfoLoader.scheduleLoad(resources, Runnable::run).join();
        var discovery = new ModelDiscovery(models, MissingCuboidModel.missingModel());
        discovery.addSpecialModel(ItemModelGenerator.GENERATED_ITEM_MODEL_ID, new ItemModelGenerator());
        states.models().values().forEach(discovery::addRoot);
        items.contents().values().forEach(info -> discovery.addRoot(info.model()));
        var resolved = discovery.resolve();
        var bakery = new ModelBakery(entityModels, atlases, null, states.models(), items.contents(), resolved, discovery.missingModel());
        var materials = new MaterialBaker(Atlases.prepared(AtlasIds.BLOCKS), Atlases.prepared(AtlasIds.ITEMS));
        baked = bakery.bakeModels(materials, Runnable::run).join();
        var blockColors = Game.blockColors();
        manager = new ModelManager(blockColors, atlases, null);
        Map<BlockState, BlockStateModel> dispatch = Shell.call(ModelManager.class, null, "createBlockStateToModelDispatch",
            new Class<?>[]{Map.class, BlockStateModel.class}, baked.blockStateModels(), baked.missingModels().block());
        Object groups = Shell.call(ModelManager.class, null, "buildModelGroups",
            new Class<?>[]{net.minecraft.client.color.block.BlockColors.class, BlockStateModelLoader.LoadedModels.class}, blockColors, states);
        Function<BlockState, BlockStateModel> byState = baked::getBlockStateModel;
        var blockModels = new LoadedBlockModels(BuiltInBlockModels.createBlockModels(blockColors), entityModels, atlases, null)
            .bake(byState, baked.missingModels().block(), Runnable::run).join();
        var fluids = FluidStateModelSet.bake(materials);
        Class<?> reloadState = Class.forName(ModelManager.class.getName() + "$ReloadState");
        var state = Shell.construct(reloadState, baked, groups, dispatch, blockModels, fluids, entityModels);
        Shell.call(ModelManager.class, manager, "apply", new Class<?>[]{reloadState}, state);
    }

    public static synchronized Models get() throws Exception {
        if (instance == null) {
            Game.init();
            instance = new Models();
        }
        return instance;
    }

    /** The state's model, or null where the resources define none (the client draws the missing model). */
    public BlockStateModel blockStateModel(BlockState state) {
        return baked.blockStateModels().get(state);
    }
}
