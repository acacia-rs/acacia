package acacia.parity.item_transforms;

import acacia.parity.Game;
import acacia.parity.Out;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.math.Axis;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.*;
import net.minecraft.client.renderer.FirstPersonHandsAndItemsRenderer;
import net.minecraft.util.Mth;
import net.minecraft.world.entity.HumanoidArm;

/**
 * First-person item placement (docs/parity.md, harness item_transforms): per case in args[1], the
 * pose FirstPersonHandsAndItemsRenderer.submitArmWithItem hands the item's render state, from an
 * identity view, as args[0]/hands.jsonl `matrix` (column-major). The game's own applyItemArmTransform,
 * swingArm and applyEatTransform run by reflection; the branches between them and the inline bow,
 * trident, crossbow and block transforms are transcribed from submitArmWithItem.
 */
public final class ItemTransformsOracle {
    private static Object renderer;

    public static void main(String[] args) throws Exception {
        Game.init();
        var unsafe = Class.forName("sun.misc.Unsafe").getDeclaredField("theUnsafe");
        unsafe.setAccessible(true);
        // Its transforms read no fields: an unconstructed instance runs them.
        var u = unsafe.get(null);
        renderer = u.getClass().getMethod("allocateInstance", Class.class).invoke(u, FirstPersonHandsAndItemsRenderer.class);
        try (var out = new Out.Lines(Path.of(args[0]).resolve("hands.jsonl"))) {
            for (String line : Files.readAllLines(Path.of(args[1]))) {
                if (line.isBlank()) continue;
                var kase = JsonParser.parseString(line).getAsJsonObject();
                var record = new TreeMap<String, Object>();
                for (var e : kase.entrySet()) record.put(e.getKey(), e.getValue());
                try {
                    var stack = new PoseStack();
                    place(stack, kase);
                    record.put("matrix", stack.last().pose().get(new float[16]));
                } catch (Throwable error) {
                    record.put("error", String.valueOf(error.getCause() != null ? error.getCause() : error));
                }
                out.write(record);
            }
        }
    }

    private static Object call(String name, Class<?>[] types, Object... args) throws Exception {
        Method m = FirstPersonHandsAndItemsRenderer.class.getDeclaredMethod(name, types);
        m.setAccessible(true);
        return m.invoke(renderer, args);
    }

    private static void armTransform(PoseStack s, HumanoidArm arm, float lowered) throws Exception {
        call("applyItemArmTransform", new Class<?>[] {PoseStack.class, HumanoidArm.class, float.class}, s, arm, lowered);
    }

    private static void swingArm(PoseStack s, float attack, int invert, HumanoidArm arm) throws Exception {
        call("swingArm", new Class<?>[] {float.class, PoseStack.class, int.class, HumanoidArm.class}, attack, s, invert, arm);
    }

    /** The bow, trident and crossbow pull after their turn (submitArmWithItem's shared tail). */
    private static void drawn(PoseStack s, int invert, float timeHeld, float power, float push) {
        if (power > 0.1F) {
            float shake = Mth.sin((timeHeld - 0.1F) * 1.3F) * (power - 0.1F);
            s.translate(shake * 0.0F, shake * 0.004F, shake * 0.0F);
        }
        s.translate(0.0F, 0.0F, power * push);
        s.scale(1.0F, 1.0F, 1.0F + power * 0.2F);
        s.rotateDegrees(Axis.YN, invert * 45.0F);
    }

    private static void place(PoseStack s, JsonObject k) throws Exception {
        boolean left = k.get("left").getAsBoolean();
        HumanoidArm arm = left ? HumanoidArm.LEFT : HumanoidArm.RIGHT;
        int invert = left ? -1 : 1;
        float lowered = k.get("lowered").getAsFloat(), attack = k.get("attack").getAsFloat();
        var using = k.has("using") && !k.get("using").isJsonNull() ? k.getAsJsonObject("using") : null;
        String use = using == null ? null : using.get("kind").getAsString();
        float held = using == null ? 0.0F : using.get("held").getAsFloat();
        if (k.has("crossbow") && !k.get("crossbow").isJsonNull()) {
            boolean charged = k.get("crossbow").getAsBoolean();
            armTransform(s, arm, lowered);
            if ("crossbow".equals(use) && !charged) {
                s.translate(invert * -0.4785682F, -0.094387F, 0.05731531F);
                s.rotateDegrees(Axis.XP, -11.935F);
                s.rotateDegrees(Axis.YP, invert * 65.3F);
                s.rotateDegrees(Axis.ZP, invert * -9.785F);
                drawn(s, invert, held, Math.min(held / using.get("charge").getAsFloat(), 1.0F), 0.04F);
            } else {
                swingArm(s, attack, invert, arm);
                if (charged && attack < 0.001F && k.get("main_hand").getAsBoolean()) {
                    s.translate(invert * -0.641864F, 0.0F, 0.0F);
                    s.rotateDegrees(Axis.YP, invert * 10.0F);
                }
            }
            return;
        }
        if (use != null) {
            switch (use) {
                case "eat" -> {
                    int duration = using.get("duration").getAsInt();
                    // currUsageTime = remaining - partial + 1 = duration - held.
                    call("applyEatTransform", new Class<?>[] {PoseStack.class, float.class, HumanoidArm.class, float.class, int.class},
                        s, 0.0F, arm, duration - held - 1.0F, duration);
                    armTransform(s, arm, lowered);
                }
                case "block" -> {
                    armTransform(s, arm, lowered);
                    s.translate(invert * -0.14142136F, 0.08F, 0.14142136F);
                    s.rotateDegrees(Axis.XP, -102.25F);
                    s.rotateDegrees(Axis.YP, invert * 13.365F);
                    s.rotateDegrees(Axis.ZP, invert * 78.05F);
                }
                case "bow" -> {
                    armTransform(s, arm, lowered);
                    s.translate(invert * -0.2785682F, 0.18344387F, 0.15731531F);
                    s.rotateDegrees(Axis.XP, -13.935F);
                    s.rotateDegrees(Axis.YP, invert * 35.3F);
                    s.rotateDegrees(Axis.ZP, invert * -9.785F);
                    float power = held / 20.0F;
                    drawn(s, invert, held, Math.min((power * power + power * 2.0F) / 3.0F, 1.0F), 0.04F);
                }
                case "trident" -> {
                    armTransform(s, arm, lowered);
                    s.translate(invert * -0.5F, 0.7F, 0.1F);
                    s.rotateDegrees(Axis.XP, -55.0F);
                    s.rotateDegrees(Axis.YP, invert * 35.3F);
                    s.rotateDegrees(Axis.ZP, invert * -9.785F);
                    drawn(s, invert, held, Math.min(held / 10.0F, 1.0F), 0.2F);
                }
                default -> armTransform(s, arm, lowered);
            }
        } else if (k.get("spinning").getAsBoolean()) {
            armTransform(s, arm, lowered);
            s.translate(invert * -0.4F, 0.8F, 0.3F);
            s.rotateDegrees(Axis.YP, invert * 65.0F);
            s.rotateDegrees(Axis.ZP, invert * -85.0F);
        } else {
            armTransform(s, arm, lowered);
            if (k.get("swinging").getAsBoolean()) swingArm(s, attack, invert, arm);
        }
    }
}
