package acacia.parity.entity_layers;

import acacia.parity.EntityModels;
import acacia.parity.Game;
import acacia.parity.Out;
import acacia.parity.RenderStates;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.minecraft.client.model.Model;
import net.minecraft.client.model.geom.ModelPart;
import com.mojang.math.Axis;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.*;
import net.minecraft.client.model.ArmedModel;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.world.entity.HumanoidArm;

/**
 * Render layers on posed models (docs/parity.md, harness entity_layers). args[1] holds
 * RenderStates cases; per case args[0]/layers.jsonl gets `hands`: for each arm, the matrix
 * ItemInHandLayer.submitArmWithItem hands the item (translateToHand, -90° x, 180° y, the fist
 * offset), under LivingEntityRenderer's root transform at yaw 0, column-major; `armor`: per slot
 * the armour layer's posed parts and drawn corners, as EntityPosesOracle's; `overlays`: the same
 * for each second layer (wool, outer clothes, the creeper's aura).
 */
public final class EntityLayersOracle {
    public static void main(String[] args) throws Exception {
        Game.init();
        try (var out = new Out.Lines(Path.of(args[0]).resolve("layers.jsonl"))) {
            for (String line : Files.readAllLines(Path.of(args[1]))) {
                if (line.isBlank()) continue;
                var kase = JsonParser.parseString(line).getAsJsonObject();
                var record = new TreeMap<String, Object>();
                for (var e : kase.entrySet()) record.put(e.getKey(), e.getValue());
                try {
                    var posed = RenderStates.pose(kase);
                    var hands = new TreeMap<String, Object>();
                    if (posed.model() instanceof ArmedModel<?> armed) {
                        for (HumanoidArm arm : HumanoidArm.values()) hands.put(arm.name(), hand(armed, posed.state(), arm));
                    }
                    record.put("hands", hands);
                    record.put("armor", armor(kase, posed.state()));
                    record.put("overlays", overlays(kase, posed.state()));
                } catch (Throwable error) {
                    var cause = error.getCause() != null ? error.getCause() : error;
                    record.put("error", cause.toString());
                }
                out.write(record);
            }
        }
    }

    private static final String SKELETON = "net.minecraft.client.model.monster.skeleton.SkeletonModel";
    private static final String SHEEP_FUR = "net.minecraft.client.model.animal.sheep.SheepFurModel";

    /**
     * Per armour slot the case's layer has (`<entity>#helmet`...), the slot's model as
     * HumanoidArmorLayer draws it: ArmorModelSet bakes it with the body model's class (BoggedModel's
     * armour is a SkeletonModel) and the dispatcher runs its setupAnim on the wearer's state.
     */
    private static Map<String, Object> armor(JsonObject kase, EntityRenderState state) throws Exception {
        var name = kase.get("model").getAsString();
        return layered(kase, state, name.endsWith(".BoggedModel") ? SKELETON : name, "helmet", "chestplate", "leggings", "boots");
    }

    /**
     * The second layers drawn over the body, each its own model posed by its own setupAnim:
     * DrownedOuterLayer and SkeletonClothingLayer (`#outer`, a SkeletonModel on strays and bogged),
     * SheepWoolLayer and SheepWoolUndercoatLayer (SheepFurModel), CreeperPowerLayer (`#armor`).
     */
    private static Map<String, Object> overlays(JsonObject kase, EntityRenderState state) throws Exception {
        var name = kase.get("model").getAsString();
        if (name.contains(".sheep.")) return layered(kase, state, SHEEP_FUR, "wool", "wool_undercoat");
        if (name.endsWith(".CreeperModel")) return layered(kase, state, name, "armor");
        return layered(kase, state, name.endsWith(".BoggedModel") || name.endsWith(".StrayModel") ? SKELETON : name, "outer");
    }

    /** Per suffix the case's layer has a sibling for (`<entity>#<suffix>`), it baked as `cls`, posed and drawn. */
    @SuppressWarnings("unchecked")
    private static Map<String, Object> layered(JsonObject kase, EntityRenderState state, String cls, String... suffixes) throws Exception {
        var out = new TreeMap<String, Object>();
        var base = kase.get("layer").getAsString().replace("#main", "#");
        for (String suffix : suffixes) {
            var layer = EntityModels.layers().get(base + suffix);
            if (layer == null) continue;
            var root = layer.getValue().bakeRoot();
            var model = (Model<EntityRenderState>) Class.forName(cls).getConstructor(ModelPart.class).newInstance(root);
            model.setupAnim(state);
            var parts = new TreeMap<String, Object>();
            EntityModels.poses("", root, parts);
            var drawn = new TreeMap<String, List<float[]>>();
            EntityModels.draw("", root, EntityModels.livingRoot(0.0F, new float[] {1, 1, 1}), drawn);
            out.put(suffix, Out.map("layer", base + suffix, "parts", parts, "drawn", drawn));
        }
        return out;
    }

    @SuppressWarnings("unchecked")
    private static float[] hand(ArmedModel<?> model, EntityRenderState state, HumanoidArm arm) {
        var stack = EntityModels.livingRoot(0.0F, new float[] {1, 1, 1});
        ((ArmedModel<EntityRenderState>) model).translateToHand(state, arm, stack);
        stack.rotateDegrees(Axis.XP, -90.0F);
        stack.rotateDegrees(Axis.YP, 180.0F);
        boolean left = arm == HumanoidArm.LEFT;
        boolean baby = state instanceof LivingEntityRenderState living && living.isBaby;
        float x = baby ? 0.0F : 1.0F, y = baby ? 1.0F : 2.0F, z = baby ? -4.5F : -10.0F;
        stack.translate((left ? -1 : 1) * x / 16.0F, y / 16.0F, z / 16.0F);
        return stack.last().pose().get(new float[16]);
    }
}
