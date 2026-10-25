package acacia.parity.item_models;

import acacia.parity.Game;
import acacia.parity.Out;
import acacia.parity.UnitAtlas;
import com.mojang.blaze3d.vertex.PoseStack;
import java.io.Reader;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Proxy;
import java.nio.file.Path;
import java.util.*;
import net.minecraft.client.model.Model;
import net.minecraft.client.model.geom.EntityModelSet;
import net.minecraft.client.renderer.Sheets;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.renderer.texture.OverlayTexture;
import net.minecraft.client.renderer.texture.SpriteLoader;
import net.minecraft.client.renderer.texture.TextureAtlas;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.renderer.texture.UvMapping;
import net.minecraft.client.resources.model.ClientItemInfoLoader;
import net.minecraft.client.resources.model.ModelBakery;
import net.minecraft.client.resources.model.ModelDiscovery;
import net.minecraft.client.resources.model.UnbakedModel;
import net.minecraft.client.resources.model.cuboid.CuboidModel;
import net.minecraft.client.resources.model.cuboid.ItemModelGenerator;
import net.minecraft.client.resources.model.cuboid.MissingCuboidModel;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.client.resources.model.geometry.ItemQuads;
import net.minecraft.client.resources.model.sprite.MaterialBaker;
import net.minecraft.client.resources.model.sprite.SpriteGetter;
import net.minecraft.client.resources.model.sprite.SpriteId;
import net.minecraft.core.Holder;
import net.minecraft.core.component.DataComponentMap;
import net.minecraft.core.component.PatchedDataComponentMap;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.data.AtlasIds;
import net.minecraft.resources.FileToIdConverter;
import net.minecraft.resources.Identifier;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.world.item.ItemDisplayContext;
import net.minecraft.world.item.ItemStack;
import org.joml.Matrix4f;
import org.joml.Vector3f;

/**
 * Items as the client draws them (docs/parity.md, harness item_models): every item's default stack
 * in every display context, resolved by its ItemModel and submitted by ItemStackRenderState.submit
 * under an identity pose. Per case `<item>|<context>` in args[0]/items.jsonl, each submitted quad
 * in submission order: its image, corners after the display transform, sprite-local UVs and the
 * ARGB the item renderer multiplies in (the layer's tint, or a special model's colour).
 * Submissions of other kinds are counted in `skipped`.
 */
public final class ItemModelsOracle {
    private static final FileToIdConverter MODELS = FileToIdConverter.json("models");
    private static final Map<Identifier, Identifier> ATLASES = Map.of(
        Sheets.BANNER_SHEET, AtlasIds.BANNER_PATTERNS,
        TextureAtlas.LOCATION_BLOCKS, AtlasIds.BLOCKS,
        TextureAtlas.LOCATION_ITEMS, AtlasIds.ITEMS,
        Sheets.CHEST_SHEET, AtlasIds.CHESTS,
        Sheets.DECORATED_POT_SHEET, AtlasIds.DECORATED_POT,
        Sheets.SHIELD_SHEET, AtlasIds.SHIELD_PATTERNS,
        Sheets.SHULKER_SHEET, AtlasIds.SHULKER_BOXES);
    private static final Map<Identifier, SpriteLoader.Preparations> ATLAS = new HashMap<>();

    public static void main(String[] args) throws Exception {
        Game.init();
        var baked = bake();
        try (var out = new Out.Lines(Path.of(args[0]).resolve("items.jsonl"))) {
            for (var id : new TreeSet<>(BuiltInRegistries.ITEM.keySet())) {
                var model = baked.itemStackModels().get(id);
                if (model == null) continue;
                for (var context : ItemDisplayContext.values()) {
                    if (context == ItemDisplayContext.NONE) continue;
                    var record = Out.map("case", id + "|" + context.getSerializedName());
                    try {
                        var state = new ItemStackRenderState();
                        state.clear();
                        field(ItemStackRenderState.class, "displayContext").set(state, context);
                        model.update(state, stack(id), null, context, null, null, 0);
                        var drawn = new Drawn();
                        state.submit(new PoseStack(), drawn.collector(), 0xF000F0, OverlayTexture.NO_OVERLAY, 0);
                        record.put("quads", drawn.quads);
                        record.put("skipped", drawn.skipped);
                    } catch (Throwable error) {
                        record.put("error", String.valueOf(error.getCause() != null ? error.getCause() : error));
                    }
                    out.write(record);
                }
            }
        }
    }

    static synchronized SpriteLoader.Preparations atlas(Identifier sheet) {
        return ATLAS.computeIfAbsent(sheet, s -> UnitAtlas.load(Objects.requireNonNull(ATLASES.get(s), s::toString), s, Set.of(), false));
    }

    /** ModelManager's discovery and ModelBakery's bake, for item models only. */
    /** Every item model, baked against the game's resources and unit atlases. */
    static ModelBakery.BakingResult bake() throws Exception {
        SpriteGetter sprites = id -> {
            var atlas = atlas(id.atlasLocation());
            return atlas.regions().getOrDefault(id.texture(), atlas.missing());
        };
        return bake(Game.resources(), sprites);
    }

    private static ModelBakery.BakingResult bake(ResourceManager resources, SpriteGetter sprites) throws Exception {
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
        var materials = new MaterialBaker(atlas(TextureAtlas.LOCATION_BLOCKS), atlas(TextureAtlas.LOCATION_ITEMS));
        return bakery.bakeModels(materials, Runnable::run).join();
    }

    /** Components are bound to a level's registries, which are absent here: the stack has none. */
    static ItemStack stack(Identifier id) throws Exception {
        var item = BuiltInRegistries.ITEM.wrapAsHolder(BuiltInRegistries.ITEM.getValue(id));
        var ctor = ItemStack.class.getDeclaredConstructor(Holder.class, int.class, PatchedDataComponentMap.class);
        ctor.setAccessible(true);
        return ctor.newInstance(item, 1, new PatchedDataComponentMap(DataComponentMap.EMPTY));
    }

    private static final Map<String, Field> FIELDS = new HashMap<>();

    static Field field(Class<?> owner, String name) throws Exception {
        var key = owner.getName() + "." + name;
        var f = FIELDS.get(key);
        if (f == null) {
            f = owner.getDeclaredField(name);
            f.setAccessible(true);
            FIELDS.put(key, f);
        }
        return f;
    }

    /** What one stack submits. */
    private static final class Drawn {
        final List<Map<String, Object>> quads = new ArrayList<>();
        final Map<String, Integer> skipped = new TreeMap<>();

        SubmitNodeCollector collector() {
            InvocationHandler handler = (proxy, method, args) -> {
                var name = method.getName();
                if (name.equals("order")) return proxy;
                if (name.equals("submitItem") && args.length == 8) {
                    item(((PoseStack) args[0]).last().pose(), (int[]) args[5], (ItemQuads) args[6]);
                    return null;
                }
                if (name.equals("submitModel") && args.length == 9 && args[3] instanceof RenderType type) {
                    model((Model<?>) args[0], args[1], (PoseStack) args[2], type, (int) args[6], (UvMapping) args[7]);
                    return null;
                }
                if (method.isDefault()) return InvocationHandler.invokeDefault(proxy, method, args);
                skipped.merge(name, 1, Integer::sum);
                return null;
            };
            return (SubmitNodeCollector) Proxy.newProxyInstance(SubmitNodeCollector.class.getClassLoader(), new Class<?>[] {SubmitNodeCollector.class}, handler);
        }

        private void item(Matrix4f pose, int[] tints, ItemQuads items) {
            for (BakedQuad quad : items.all()) {
                var pos = new float[12];
                var uv = new float[8];
                for (int i = 0; i < 4; i++) {
                    var p = pose.transformPosition(new Vector3f(quad.position(i)));
                    pos[i * 3] = p.x; pos[i * 3 + 1] = p.y; pos[i * 3 + 2] = p.z;
                    long packed = quad.packedUV(i);
                    uv[i * 2] = Float.intBitsToFloat((int) (packed >>> 32));
                    uv[i * 2 + 1] = Float.intBitsToFloat((int) packed);
                }
                int tint = quad.materialInfo().tintIndex();
                add(image(quad.materialInfo().sprite()), pos, uv, tint >= 0 && tint < tints.length ? tints[tint] : -1);
            }
        }

        @SuppressWarnings("unchecked")
        private void model(Model<?> model, Object state, PoseStack pose, RenderType type, int color, UvMapping mapping) throws Exception {
            ((Model<Object>) model).setupAnim(state);
            var capture = new QuadCapture((pos, uv) -> add(mapping instanceof TextureAtlasSprite sprite ? image(sprite) : Textures.of(type), pos, uv, color));
            model.renderToBuffer(pose, mapping == null ? capture : mapping.wrap(capture), 0xF000F0, OverlayTexture.NO_OVERLAY, color);
            capture.finish();
            model.resetPose();
        }

        private void add(String image, float[] pos, float[] uv, int argb) {
            quads.add(Out.map("image", image, "pos", pos, "uv", uv, "argb", argb));
        }

        private static String image(TextureAtlasSprite sprite) {
            var name = sprite.contents().name();
            return name.withPath("textures/" + name.getPath() + ".png").toString();
        }
    }
}
