package acacia.parity.entity_gear;

import acacia.parity.Client;
import acacia.parity.Headless;
import acacia.parity.Out;
import acacia.parity.Submits;
import com.mojang.blaze3d.vertex.PoseStack;
import java.nio.file.Path;
import java.util.*;
import net.minecraft.client.renderer.entity.AgeableMobRenderer;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.client.renderer.entity.layers.RenderLayer;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.animal.equine.Markings;
import net.minecraft.world.item.DyeColor;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.component.DyedItemColor;

/**
 * What the render layers of the kinds that wear saddles, body armour, carpets, harnesses, collars
 * and markings submit (docs/parity.md, harness entity_gear), args[0]/gear.jsonl: per kind and
 * seed, the gear set on a new entity's render state (`gear`) and each model a layer submitted, in
 * order: the model layers whose geometry it has, the texture, the colour and the render type.
 */
public final class EntityGearOracle {
    static final String[] KINDS = {"pig", "strider", "horse", "donkey", "mule", "skeleton_horse", "zombie_horse", "camel", "camel_husk",
        "nautilus", "zombie_nautilus", "happy_ghast", "wolf", "llama", "trader_llama", "cat"};
    static final String[] BODY = {"", "", "leather_horse_armor", "iron_horse_armor", "golden_horse_armor", "diamond_horse_armor",
        "copper_horse_armor", "netherite_horse_armor", "wolf_armor", "white_carpet", "red_carpet", "black_carpet", "white_harness",
        "blue_harness", "iron_nautilus_armor", "golden_nautilus_armor", "diamond_nautilus_armor", "netherite_nautilus_armor",
        "copper_nautilus_armor", "diamond_sword"};
    static final int SEEDS = 150;

    public static void main(String[] args) throws Exception {
        Client.minecraft();
        var level = Client.overworld();
        Headless.set(Headless.get(level, "connection"), "scoreboard", new net.minecraft.world.scores.Scoreboard());
        var dispatcher = Client.entityRenderers();
        var layers = Submits.layerPrints();
        try (var out = new Out.Lines(Path.of(args[0]).resolve("gear.jsonl"))) {
            for (String kind : KINDS) {
                var type = BuiltInRegistries.ENTITY_TYPE.getValue(Identifier.withDefaultNamespace(kind));
                for (int seed = 0; seed < SEEDS; seed++) {
                    var record = new TreeMap<String, Object>();
                    record.put("case", "minecraft:" + kind + "|" + seed);
                    record.put("kind", "minecraft:" + kind);
                    try {
                        Entity entity = type.create(level, EntitySpawnReason.LOAD);
                        entity.setId(1_000_000);
                        var renderer = (LivingEntityRenderer<?, ?, ?>) dispatcher.getRenderer(entity);
                        @SuppressWarnings("unchecked")
                        var state = (LivingEntityRenderState) ((EntityRenderer<Entity, ?>) (EntityRenderer<?, ?>) renderer).createRenderState(entity, 1.0F);
                        record.put("gear", dress(state, new Random(seed * 31L + kind.hashCode())));
                        // AgeableMobRenderer.submit's swap: layers drawing their parent's model draw the baby's.
                        if (renderer instanceof AgeableMobRenderer<?, ?, ?>)
                            Headless.set(renderer, "model", Headless.get(renderer, state.isBaby ? "babyModel" : "adultModel"));
                        var draws = new ArrayList<Map<String, Object>>();
                        var collector = Submits.collector(draws, layers);
                        @SuppressWarnings("unchecked")
                        var list = (List<RenderLayer<LivingEntityRenderState, ?>>) Headless.get(renderer, "layers");
                        for (var layer : list) layer.submit(new PoseStack(), collector, 0xF000F0, state, 0.0F, 0.0F);
                        record.put("draws", draws);
                    } catch (Throwable error) {
                        var cause = error.getCause() != null ? error.getCause() : error;
                        record.put("error", cause + " at " + Arrays.toString(Arrays.copyOf(cause.getStackTrace(), 4)));
                    }
                    out.write(record);
                }
            }
        }
    }

    /** Sets random gear on `state` where it has the field; returns what was set. */
    private static Map<String, Object> dress(LivingEntityRenderState state, Random r) {
        var gear = new TreeMap<String, Object>();
        boolean baby = r.nextInt(4) == 0, invisible = r.nextInt(8) == 0;
        state.isBaby = baby;
        state.isInvisible = invisible;
        gear.put("baby", baby);
        gear.put("invisible", invisible);
        if (put(state, "saddle", r.nextBoolean() ? new ItemStack(Items.SADDLE) : ItemStack.EMPTY)) gear.put("saddled", !((ItemStack) Headless.get(state, "saddle")).isEmpty());
        var body = item(BODY[r.nextInt(BODY.length)]);
        if (body.isDamageableItem() && r.nextBoolean()) body.setDamageValue(r.nextInt(body.getMaxDamage()));
        if (body.is(Items.LEATHER_HORSE_ARMOR) || body.is(Items.WOLF_ARMOR)) {
            if (r.nextBoolean()) body.set(DataComponents.DYED_COLOR, new DyedItemColor(r.nextInt(0x1000000)));
        }
        if (put(state, "bodyArmorItem", body) || put(state, "bodyItem", body)) {
            gear.put("body", body.isEmpty() ? "" : BuiltInRegistries.ITEM.getKey(body.getItem()).toString());
            gear.put("damage", body.getDamageValue());
            gear.put("maxDamage", body.getMaxDamage());
            var dye = body.get(DataComponents.DYED_COLOR);
            if (dye != null) gear.put("dye", dye.rgb() & 0xFFFFFF);
        }
        DyeColor collar = r.nextInt(3) == 0 ? null : DyeColor.byId(r.nextInt(16));
        if (put(state, "collarColor", collar)) gear.put("collar", collar == null ? -1 : collar.getId());
        Markings markings = Markings.values()[r.nextInt(Markings.values().length)];
        if (put(state, "markings", markings)) gear.put("markings", markings.ordinal());
        boolean holder = r.nextBoolean();
        if (put(state, "isLeashHolder", holder)) gear.put("leashHolder", holder);
        boolean ridden = r.nextBoolean();
        if (put(state, "isRidden", ridden)) gear.put("ridden", ridden);
        return gear;
    }

    private static ItemStack item(String id) {
        if (id.isEmpty()) return ItemStack.EMPTY;
        return BuiltInRegistries.ITEM.getOptional(Identifier.withDefaultNamespace(id)).map(ItemStack::new).orElse(ItemStack.EMPTY);
    }

    private static boolean put(Object state, String name, Object value) {
        for (Class<?> c = state.getClass(); c != null; c = c.getSuperclass()) {
            try {
                c.getDeclaredField(name);
                Headless.set(state, name, value);
                return true;
            } catch (NoSuchFieldException e) {
            }
        }
        return false;
    }

}
