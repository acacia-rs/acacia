package acacia.parity.entity_layers;

import acacia.parity.EntityModels;
import acacia.parity.Game;
import acacia.parity.Out;
import acacia.parity.RenderStates;
import com.google.gson.JsonParser;
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
 * offset), under LivingEntityRenderer's root transform at yaw 0, column-major.
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
                } catch (Throwable error) {
                    var cause = error.getCause() != null ? error.getCause() : error;
                    record.put("error", cause.toString());
                }
                out.write(record);
            }
        }
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
