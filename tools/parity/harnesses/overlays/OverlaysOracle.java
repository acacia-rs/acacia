package acacia.parity.overlays;

import acacia.parity.Client;
import acacia.parity.Headless;
import acacia.parity.Out;
import com.mojang.blaze3d.vertex.PoseStack;
import java.lang.reflect.Method;
import java.nio.file.Path;
import java.util.*;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.client.renderer.entity.state.CreeperRenderState;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;

/**
 * The entity colour overlay and shaking (docs/parity.md, harness overlays),
 * args[0]/overlays.jsonl:
 * `overlay|<hurt>|<progress>`: LivingEntityRenderer.getOverlayCoords as (u, v), and the texel
 * OverlayTexture's constructor writes there (transcribed: the texture needs a GPU), as ARGB;
 * `creeper|<swelling>`: CreeperRenderer.getWhiteOverlayProgress;
 * `shake|<type>|<frozen>|<converting>|<suffocating>|<age>`: the renderer's isShaking on the
 * state extracted from a new entity with those flags set, and the matrix setupRotations makes
 * from a body yaw of 0 (column-major).
 */
public final class OverlaysOracle {
    static final String[] KINDS = {"zombie", "husk", "drowned", "zombie_villager", "piglin", "hoglin", "skeleton", "stray", "bogged",
        "wither_skeleton", "strider", "cow", "creeper", "spider", "sheep", "chicken", "pig"};

    public static void main(String[] args) throws Exception {
        Client.minecraft();
        var level = Client.overworld();
        Headless.set(Headless.get(level, "connection"), "scoreboard", new net.minecraft.world.scores.Scoreboard());
        var dispatcher = Client.entityRenderers();
        try (var out = new Out.Lines(Path.of(args[0]).resolve("overlays.jsonl"))) {
            for (boolean hurt : new boolean[] {false, true}) {
                for (int i = 0; i <= 40; i++) {
                    float progress = i / 40.0F;
                    var state = new LivingEntityRenderState();
                    state.hasRedOverlay = hurt;
                    int packed = LivingEntityRenderer.getOverlayCoords(state, progress);
                    int u = packed & 0xFFFF, v = packed >>> 16;
                    int texel = v < 8 ? -1291911168 : net.minecraft.util.ARGB.white((int) ((1.0F - u / 15.0F * 0.75F) * 255.0F));
                    out.write(Out.map("case", "overlay|" + hurt + "|" + progress, "hurt", hurt, "progress", progress, "u", u, "v", v, "texel", Integer.toUnsignedLong(texel)));
                }
            }
            var creeper = dispatcher.getRenderer(Client.spawn(level, net.minecraft.world.entity.EntityTypes.CREEPER, 0.5, 70, 0.5));
            Method white = find(creeper.getClass(), "getWhiteOverlayProgress");
            for (int i = 0; i <= 100; i++) {
                var state = new CreeperRenderState();
                state.swelling = i / 100.0F;
                out.write(Out.map("case", "creeper|" + state.swelling, "swelling", state.swelling, "white", white.invoke(creeper, state)));
            }
            for (String kind : KINDS) {
                var type = BuiltInRegistries.ENTITY_TYPE.getValue(Identifier.withDefaultNamespace(kind));
                Entity entity = type.create(level, EntitySpawnReason.LOAD);
                entity.setId(1_000_000);
                var renderer = dispatcher.getRenderer(entity);
                Method shaking = find(renderer.getClass(), "isShaking");
                Method rotations = find(renderer.getClass(), "setupRotations");
                for (int flags = 0; flags < 8; flags++) {
                    for (float age : new float[] {0.0F, 7.5F, 33.9F}) {
                        @SuppressWarnings("unchecked")
                        var state = (LivingEntityRenderState) ((EntityRenderer<Entity, ?>) renderer).createRenderState(entity, 1.0F);
                        boolean frozen = (flags & 1) != 0, converting = (flags & 2) != 0, suffocating = (flags & 4) != 0;
                        state.isFullyFrozen = frozen;
                        set(state, "isConverting", converting);
                        set(state, "isShaking", converting);
                        set(state, "isSuffocating", suffocating);
                        state.ageInTicks = age;
                        var stack = new PoseStack();
                        rotations.invoke(renderer, state, stack, 0.0F, 1.0F);
                        out.write(Out.map("case", "shake|minecraft:" + kind + "|" + frozen + "|" + converting + "|" + suffocating + "|" + age,
                            "kind", "minecraft:" + kind, "frozen", frozen, "converting", converting, "suffocating", suffocating, "age", age,
                            "shaking", shaking.invoke(renderer, state), "matrix", stack.last().pose().get(new float[16])));
                    }
                }
            }
        }
    }

    private static void set(EntityRenderState state, String field, boolean value) {
        try {
            Headless.set(state, field, value);
        } catch (IllegalArgumentException absent) {
        }
    }

    private static Method find(Class<?> type, String name) {
        for (Class<?> c = type; c != null; c = c.getSuperclass())
            for (Method m : c.getDeclaredMethods())
                if (m.getName().equals(name) && !m.isBridge()) {
                    m.setAccessible(true);
                    return m;
                }
        throw new IllegalArgumentException(type + " has no " + name);
    }
}
