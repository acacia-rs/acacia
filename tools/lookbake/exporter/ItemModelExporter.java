// SPDX-License-Identifier: MIT
package acacia.lookbake;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.io.Reader;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.client.ClientBootstrap;
import net.minecraft.client.model.geom.EntityModelSet;
import net.minecraft.client.renderer.item.ItemModel;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.client.renderer.special.SpecialModelRenderer;
import net.minecraft.client.renderer.texture.TextureAtlas;
import net.minecraft.client.resources.model.ClientItemInfoLoader;
import net.minecraft.client.resources.model.ModelBakery;
import net.minecraft.client.resources.model.ModelDiscovery;
import net.minecraft.client.resources.model.UnbakedModel;
import net.minecraft.client.resources.model.cuboid.CuboidModel;
import net.minecraft.client.resources.model.cuboid.ItemModelGenerator;
import net.minecraft.client.resources.model.cuboid.ItemTransform;
import net.minecraft.client.resources.model.cuboid.MissingCuboidModel;
import net.minecraft.client.resources.model.geometry.ItemQuads;
import net.minecraft.client.resources.model.sprite.MaterialBaker;
import net.minecraft.core.Holder;
import net.minecraft.core.component.DataComponentMap;
import net.minecraft.core.component.PatchedDataComponentMap;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.FileToIdConverter;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.packs.PackLocationInfo;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.VanillaPackResourcesBuilder;
import net.minecraft.server.packs.repository.BuiltInPackSource;
import net.minecraft.server.packs.repository.PackSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.world.item.ItemDisplayContext;
import net.minecraft.world.item.ItemStack;
import org.joml.Matrix4f;

/**
 * Writes how the client draws every item's default stack, in every display context: the item
 * models (items/*.json) baked as ModelManager bakes them, each resolved by ItemModel.update with
 * no level and no holder. Per item and context, its layers: geometry (ItemGeometry, shared across
 * items in `meshes`), tint colours, foil, the display transform (rotation in degrees, translation
 * in blocks, scale), whether it is lit as a block, and the special renderer's class. Run by
 * `lookbake java` against the user's own client jar (docs/java-look.md "Licensing").
 *
 * usage: java -cp <client jar>:<its libraries>:<classes> acacia.lookbake.ItemModelExporter OUT.json
 */
public final class ItemModelExporter {
    private static final FileToIdConverter MODELS = FileToIdConverter.json("models");

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        ClientBootstrap.bootstrap();
        var resources = vanilla();
        var sprites = new UnitSprites(resources);
        var baked = bake(resources, sprites);
        var meshes = new JsonArray();
        var meshIndex = new HashMap<String, Integer>();
        var items = new JsonObject();
        int failed = 0;
        for (var id : new TreeSet<>(BuiltInRegistries.ITEM.keySet())) {
            var model = baked.itemStackModels().get(id);
            if (model == null) continue;
            var item = new JsonObject();
            try {
                var stack = stack(id);
                for (var context : ItemDisplayContext.values()) item.add(context.getSerializedName(), layers(model, stack, context, meshes, meshIndex));
            } catch (Exception | LinkageError error) {
                item = new JsonObject();
                item.addProperty("error", error.toString());
                failed++;
            }
            items.add(id.toString(), item);
        }
        var out = new JsonObject();
        out.add("meshes", meshes);
        out.add("items", items);
        Files.writeString(Path.of(args[0]), new Gson().toJson(out), StandardCharsets.UTF_8);
        System.out.println(items.size() + " items, " + meshes.size() + " meshes, " + failed + " failed");
    }

    private static ResourceManager vanilla() {
        var info = new PackLocationInfo(BuiltInPackSource.VANILLA_ID, Component.literal("vanilla"), PackSource.BUILT_IN,
            Optional.of(BuiltInPackSource.CORE_PACK_INFO));
        var pack = new VanillaPackResourcesBuilder().exposeNamespace("minecraft", "realms").pushJarResources().build(info);
        return new MultiPackResourceManager(PackType.CLIENT_RESOURCES, List.of(pack.fullResources()));
    }

    /** ModelManager's discovery and ModelBakery's bake, for item models only. */
    private static ModelBakery.BakingResult bake(ResourceManager resources, UnitSprites sprites) throws Exception {
        Map<Identifier, UnbakedModel> models = new HashMap<>();
        for (var entry : MODELS.listMatchingResources(resources).entrySet()) {
            try (Reader reader = entry.getValue().openAsReader()) {
                models.put(MODELS.fileToId(entry.getKey()), CuboidModel.fromStream(reader));
            }
        }
        var infos = ClientItemInfoLoader.scheduleLoad(resources, Runnable::run).join().contents();
        var discovery = new ModelDiscovery(models, MissingCuboidModel.missingModel());
        discovery.addSpecialModel(ItemModelGenerator.GENERATED_ITEM_MODEL_ID, new ItemModelGenerator());
        infos.values().forEach(info -> discovery.addRoot(info.model()));
        var resolved = discovery.resolve();
        var bakery = new ModelBakery(EntityModelSet.vanilla(), sprites, null, Map.of(), infos, resolved, discovery.missingModel());
        var materials = new MaterialBaker(sprites.atlas(TextureAtlas.LOCATION_BLOCKS), sprites.atlas(TextureAtlas.LOCATION_ITEMS));
        return bakery.bakeModels(materials, Runnable::run).join();
    }

    /** The item's stack without components: level-bound defaults are unavailable here. */
    private static ItemStack stack(Identifier id) throws Exception {
        var item = BuiltInRegistries.ITEM.wrapAsHolder(BuiltInRegistries.ITEM.getValue(id));
        var ctor = ItemStack.class.getDeclaredConstructor(Holder.class, int.class, PatchedDataComponentMap.class);
        ctor.setAccessible(true);
        return ctor.newInstance(item, 1, new PatchedDataComponentMap(DataComponentMap.EMPTY));
    }

    private static JsonArray layers(ItemModel model, ItemStack stack, ItemDisplayContext context, JsonArray meshes,
                                    Map<String, Integer> meshIndex) throws Exception {
        var state = new ItemStackRenderState();
        state.clear();
        field(ItemStackRenderState.class, "displayContext").set(state, context);
        model.update(state, stack, null, context, null, null, 0);
        int count = (int) field(ItemStackRenderState.class, "activeLayerCount").get(state);
        var all = (Object[]) field(ItemStackRenderState.class, "layers").get(state);
        var out = new JsonArray();
        for (int i = 0; i < count; i++) out.add(layer(all[i], meshes, meshIndex));
        return out;
    }

    @SuppressWarnings("unchecked")
    private static JsonObject layer(Object layer, JsonArray meshes, Map<String, Integer> meshIndex) throws Exception {
        Class<?> type = layer.getClass();
        var local = (Matrix4f) field(type, "localTransform").get(layer);
        var geometry = new ItemGeometry();
        var special = (SpecialModelRenderer<Object>) field(type, "specialRenderer").get(layer);
        if (special != null) geometry.special(special, field(type, "argumentForSpecialRendering").get(layer), local);
        else geometry.baked(((ItemQuads) field(type, "quads").get(layer)).all(), local);
        var mesh = new JsonObject();
        var quads = new JsonArray();
        geometry.quads.forEach(quads::add);
        mesh.add("quads", quads);
        var skipped = new JsonObject();
        geometry.skipped.forEach(skipped::addProperty);
        mesh.add("skipped", skipped);
        var key = mesh.toString();
        var index = meshIndex.computeIfAbsent(key, k -> {
            meshes.add(mesh);
            return meshes.size() - 1;
        });
        var out = new JsonObject();
        out.addProperty("mesh", index);
        var tints = new JsonArray();
        var tintLayers = (Iterable<Integer>) field(type, "tintLayers").get(layer);
        if (tintLayers != null) tintLayers.forEach(tints::add);
        out.add("tints", tints);
        out.addProperty("foil", field(type, "foilType").get(layer).toString());
        var transform = (ItemTransform) field(type, "itemTransform").get(layer);
        var r = transform.rotation(); var t = transform.translation(); var s = transform.scale();
        out.add("transform", ItemGeometry.array(new float[] {r.x(), r.y(), r.z(), t.x(), t.y(), t.z(), s.x(), s.y(), s.z()}));
        out.addProperty("block_light", (boolean) field(type, "usesBlockLight").get(layer));
        if (special != null) out.addProperty("special", special.getClass().getSimpleName());
        return out;
    }

    private static final Map<String, Field> FIELDS = new HashMap<>();

    private static Field field(Class<?> owner, String name) throws Exception {
        var key = owner.getName() + "." + name;
        var f = FIELDS.get(key);
        if (f == null) {
            f = owner.getDeclaredField(name);
            f.setAccessible(true);
            FIELDS.put(key, f);
        }
        return f;
    }
}
