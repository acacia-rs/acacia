package acacia.parity.item_models;

import acacia.parity.Game;
import acacia.parity.Out;
import java.nio.file.Path;
import java.util.*;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.client.renderer.item.ItemModel;
import net.minecraft.client.resources.model.ModelBakery;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.client.resources.model.geometry.ItemQuads;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderOwner;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.item.equipment.trim.ArmorTrim;
import net.minecraft.world.item.equipment.trim.TrimMaterial;
import net.minecraft.world.item.equipment.trim.TrimMaterials;
import net.minecraft.world.item.equipment.trim.TrimPattern;
import net.minecraft.world.item.equipment.trim.TrimPatterns;
import net.minecraft.core.component.DataComponentMap;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.component.PatchedDataComponentMap;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.ItemDisplayContext;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.alchemy.PotionContents;

/**
 * Items whose look follows a stack component (docs/parity.md, harness item_models),
 * args[0]/item_components.jsonl: `potion|<item>|<potion>`: each potion-holding item with
 * PotionContents of every registered potion (and none), resolved for GUI by its ItemModel: per
 * layer, the tints its tint sources give.
 */
public final class ItemComponentsOracle {
    private static final String[] POTION_ITEMS = {"potion", "splash_potion", "lingering_potion", "tipped_arrow"};

    public static void main(String[] args) throws Exception {
        Game.init();
        var baked = ItemModelsOracle.bake();
        var potions = new ArrayList<Optional<? extends Holder<net.minecraft.world.item.alchemy.Potion>>>();
        potions.add(Optional.empty());
        BuiltInRegistries.POTION.listElements().forEach(h -> potions.add(Optional.of(h)));
        try (var out = new Out.Lines(Path.of(args[0]).resolve("item_components.jsonl"))) {
            for (String name : POTION_ITEMS) {
                var id = Identifier.withDefaultNamespace(name);
                var model = baked.itemStackModels().get(id);
                for (var potion : potions) {
                    var potionId = potion.map(h -> h.unwrapKey().orElseThrow().identifier().toString()).orElse("none");
                    var record = Out.map("case", "potion|" + id + "|" + potionId, "item", id.toString(), "potion", potionId);
                    try {
                        var components = new PatchedDataComponentMap(DataComponentMap.EMPTY);
                        @SuppressWarnings("unchecked")
                        var contents = new PotionContents((Optional<Holder<net.minecraft.world.item.alchemy.Potion>>) (Optional<?>) potion, Optional.empty(), List.of(), Optional.empty());
                        components.set(DataComponents.POTION_CONTENTS, contents);
                        var item = BuiltInRegistries.ITEM.wrapAsHolder(BuiltInRegistries.ITEM.getValue(id));
                        var ctor = ItemStack.class.getDeclaredConstructor(Holder.class, int.class, PatchedDataComponentMap.class);
                        ctor.setAccessible(true);
                        var stack = ctor.newInstance(item, 1, components);
                        var state = new ItemStackRenderState();
                        state.clear();
                        ItemModelsOracle.field(ItemStackRenderState.class, "displayContext").set(state, ItemDisplayContext.GUI);
                        model.update(state, stack, null, ItemDisplayContext.GUI, null, null, 0);
                        record.put("tints", tints(state));
                        record.put("color", contents.getColor());
                    } catch (Throwable error) {
                        record.put("error", String.valueOf(error.getCause() != null ? error.getCause() : error));
                    }
                    out.write(record);
                }
            }
            trims(baked, out);
        }
    }

    private static final HolderOwner<TrimMaterial> MATERIALS = new HolderOwner<>() {};
    private static final HolderOwner<TrimPattern> PATTERNS = new HolderOwner<>() {};

    /**
     * `trim|<item>|<material>`: every item whose GUI images change with an ArmorTrim of a
     * TrimMaterials material, per layer the images its quads sample, in order of first use.
     */
    private static void trims(ModelBakery.BakingResult baked, Out.Lines out) throws Exception {
        var pattern = Holder.Reference.createStandAlone(PATTERNS, TrimPatterns.COAST);
        for (var id : new TreeSet<>(BuiltInRegistries.ITEM.keySet())) {
            var model = baked.itemStackModels().get(id);
            if (model == null) continue;
            var plain = images(model, ItemModelsOracle.stack(id));
            for (var field : TrimMaterials.class.getFields()) {
                if (!(field.get(null) instanceof ResourceKey<?> key)) continue;
                @SuppressWarnings("unchecked")
                var material = Holder.Reference.createStandAlone(MATERIALS, (ResourceKey<TrimMaterial>) key);
                var stack = ItemModelsOracle.stack(id);
                stack.set(DataComponents.TRIM, new ArmorTrim(material, pattern));
                var images = images(model, stack);
                if (images.equals(plain)) continue;
                var name = key.identifier().getPath();
                out.write(Out.map("case", "trim|" + id + "|" + name, "item", id.toString(), "material", name, "images", images));
            }
        }
    }

    private static List<List<String>> images(ItemModel model, ItemStack stack) throws Exception {
        var state = new ItemStackRenderState();
        state.clear();
        ItemModelsOracle.field(ItemStackRenderState.class, "displayContext").set(state, ItemDisplayContext.GUI);
        model.update(state, stack, null, ItemDisplayContext.GUI, null, null, 0);
        int count = (int) ItemModelsOracle.field(ItemStackRenderState.class, "activeLayerCount").get(state);
        var all = (Object[]) ItemModelsOracle.field(ItemStackRenderState.class, "layers").get(state);
        var out = new ArrayList<List<String>>();
        for (int i = 0; i < count; i++) {
            var names = new LinkedHashSet<String>();
            if (ItemModelsOracle.field(all[i].getClass(), "quads").get(all[i]) instanceof ItemQuads quads) {
                for (BakedQuad quad : quads.all()) {
                    var sprite = quad.materialInfo().sprite().contents().name();
                    names.add(sprite.withPath("textures/" + sprite.getPath() + ".png").toString());
                }
            }
            out.add(new ArrayList<>(names));
        }
        return out;
    }

    /** Per active layer, its tint layers as ARGB. */
    static List<int[]> tints(ItemStackRenderState state) throws Exception {
        int count = (int) ItemModelsOracle.field(ItemStackRenderState.class, "activeLayerCount").get(state);
        var all = (Object[]) ItemModelsOracle.field(ItemStackRenderState.class, "layers").get(state);
        var out = new ArrayList<int[]>();
        for (int i = 0; i < count; i++) {
            var list = (it.unimi.dsi.fastutil.ints.IntList) ItemModelsOracle.field(all[i].getClass(), "tintLayers").get(all[i]);
            out.add(list == null ? new int[0] : list.toIntArray());
        }
        return out;
    }
}
