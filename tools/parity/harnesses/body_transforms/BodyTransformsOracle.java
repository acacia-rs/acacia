package acacia.parity.body_transforms;

import acacia.parity.Client;
import acacia.parity.Headless;
import acacia.parity.Out;
import com.mojang.blaze3d.vertex.PoseStack;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Path;
import java.util.*;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.core.Direction;
import net.minecraft.world.entity.player.PlayerModelType;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.Pose;

/**
 * The whole-body transform LivingEntityRenderer.submit gives a model (docs/parity.md, harness
 * body_transforms), args[0]/body.jsonl: per kind and seed, the render state extracted from a new
 * entity with random fields set (`state`: every field setupRotations or scale reads, as it ended
 * up), and the pose matrix (column-major) after the entity's scale, the renderer's
 * setupRotations, scale(-1, -1, 1), its scale and the 1.501 drop, the bed offset first.
 */
public final class BodyTransformsOracle {
    static final String[] KINDS = {"pig", "zombie", "drowned", "skeleton", "spider", "cave_spider", "silverfish", "endermite", "armor_stand",
        "squid", "glow_squid", "pufferfish", "shulker", "cat", "cod", "tropical_fish", "salmon", "fox", "iron_golem", "panda", "phantom",
        "creeper", "wither", "slime", "magma_cube", "sulfur_cube", "player", "strider", "cow", "villager"};
    /** Every state field the transform reads; written back as they ended up. */
    static final String[] FIELDS = {"scale", "bodyRot", "ageInTicks", "xRot", "deathTime", "isAutoSpinAttack", "pose", "bedOrientation",
        "eyeHeight", "isUpsideDown", "boundingBoxHeight", "isFullyFrozen", "isConverting", "isShaking", "isSuffocating", "isBaby", "wiggle",
        "lieDownAmount", "isLyingOnTopOfSleepingPlayer", "isInWater", "swelling", "swimAmount", "isPouncing", "isFaceplanted",
        "walkAnimationSpeed", "walkAnimationPos", "rollTime", "sitAmount", "isScared", "lieOnBackAmount", "size", "squish", "xBodyRot",
        "zBodyRot", "attachFace", "invulnerableTicks", "isFallFlying", "fallFlyingTimeInTicks", "shouldApplyFlyingYRot", "flyingYRot",
        "isVisuallySwimming", "fuseRemainingTicks", "isInvisible"};
    static final int SEEDS = 48;

    @SuppressWarnings("unchecked")
    public static void main(String[] args) throws Exception {
        Client.minecraft();
        var level = Client.overworld();
        Headless.set(Headless.get(level, "connection"), "scoreboard", new net.minecraft.world.scores.Scoreboard());
        var dispatcher = Client.entityRenderers();
        try (var out = new Out.Lines(Path.of(args[0]).resolve("body.jsonl"))) {
            for (String kind : KINDS) {
                for (int seed = 0; seed < SEEDS; seed++) {
                    var record = new TreeMap<String, Object>();
                    record.put("case", "minecraft:" + kind + "|" + seed);
                    record.put("kind", "minecraft:" + kind);
                    try {
                        EntityRenderer<?, ?> renderer;
                        LivingEntityRenderState state;
                        if (kind.equals("player")) {
                            // No player entity outside a connection: the wide avatar renderer's fresh state.
                            renderer = ((Map<PlayerModelType, ? extends EntityRenderer<?, ?>>) Headless.get(dispatcher, "playerRenderers")).get(PlayerModelType.WIDE);
                            state = (LivingEntityRenderState) Headless.call(EntityRenderer.class, renderer, "createRenderState", new Class<?>[0]);
                        } else {
                            var type = BuiltInRegistries.ENTITY_TYPE.getValue(Identifier.withDefaultNamespace(kind));
                            Entity entity = type.create(level, EntitySpawnReason.LOAD);
                            entity.setId(1_000_000);
                            renderer = dispatcher.getRenderer(entity);
                            state = (LivingEntityRenderState) ((EntityRenderer<Entity, ?>) renderer).createRenderState(entity, 1.0F);
                        }
                        randomize(state, new Random(seed * 7919L + kind.hashCode()));
                        var stack = new PoseStack();
                        if (state.hasPose(Pose.SLEEPING) && state.bedOrientation != null) {
                            float head = state.eyeHeight - 0.1F;
                            stack.translate(-state.bedOrientation.getStepX() * head, 0.0F, -state.bedOrientation.getStepZ() * head);
                        }
                        float scale = state.scale;
                        stack.scale(scale, scale, scale);
                        find(renderer.getClass(), "setupRotations").invoke(renderer, state, stack, state.bodyRot, scale);
                        stack.scale(-1.0F, -1.0F, 1.0F);
                        find(renderer.getClass(), "scale").invoke(renderer, state, stack);
                        stack.translate(0.0F, -1.501F, 0.0F);
                        record.put("state", read(state));
                        record.put("matrix", stack.last().pose().get(new float[16]));
                    } catch (Throwable error) {
                        var cause = error.getCause() != null ? error.getCause() : error;
                        record.put("error", cause + " at " + Arrays.toString(Arrays.copyOf(cause.getStackTrace(), 4)));
                    }
                    out.write(record);
                }
            }
        }
    }

    private static void randomize(LivingEntityRenderState state, Random r) {
        var horizontal = new Direction[] {Direction.NORTH, Direction.SOUTH, Direction.WEST, Direction.EAST};
        put(state, "bodyRot", r.nextFloat() * 360.0F - 180.0F);
        put(state, "ageInTicks", r.nextFloat() * 100.0F);
        put(state, "xRot", r.nextFloat() * 120.0F - 60.0F);
        put(state, "scale", new float[] {1.0F, 1.0F, 0.5F, 1.5F}[r.nextInt(4)]);
        // A dying entity's deathTime is at least 1 (the fall's square root is NaN below).
        put(state, "deathTime", r.nextInt(5) == 0 ? 1.0F + r.nextFloat() * 24.0F : 0.0F);
        put(state, "isAutoSpinAttack", r.nextInt(8) == 0);
        if (r.nextInt(8) == 0) {
            state.pose = Pose.SLEEPING;
            state.bedOrientation = r.nextInt(3) == 0 ? null : horizontal[r.nextInt(4)];
        }
        put(state, "eyeHeight", r.nextFloat() * 1.6F);
        put(state, "isUpsideDown", r.nextInt(8) == 0);
        put(state, "boundingBoxHeight", r.nextFloat() * 2.0F + 0.3F);
        put(state, "isFullyFrozen", r.nextInt(6) == 0);
        boolean converting = r.nextInt(8) == 0;
        put(state, "isConverting", converting);
        put(state, "isShaking", converting);
        put(state, "isBaby", r.nextInt(4) == 0);
        put(state, "wiggle", r.nextFloat() * 10.0F);
        put(state, "lieDownAmount", r.nextInt(3) == 0 ? r.nextFloat() : 0.0F);
        put(state, "isLyingOnTopOfSleepingPlayer", r.nextBoolean());
        put(state, "isInWater", r.nextBoolean());
        put(state, "swelling", r.nextFloat() * 1.2F);
        put(state, "swimAmount", r.nextInt(3) == 0 ? r.nextFloat() : 0.0F);
        put(state, "isPouncing", r.nextInt(4) == 0);
        put(state, "isFaceplanted", r.nextInt(6) == 0);
        put(state, "walkAnimationSpeed", r.nextInt(4) == 0 ? 0.0F : r.nextFloat());
        put(state, "walkAnimationPos", r.nextFloat() * 40.0F);
        put(state, "rollTime", r.nextInt(3) == 0 ? r.nextFloat() * 34.0F : 0.0F);
        put(state, "sitAmount", r.nextInt(3) == 0 ? r.nextFloat() : 0.0F);
        put(state, "isScared", r.nextBoolean());
        put(state, "lieOnBackAmount", r.nextInt(3) == 0 ? r.nextFloat() : 0.0F);
        put(state, "size", 1 + r.nextInt(4));
        put(state, "squish", r.nextFloat() - 0.5F);
        put(state, "xBodyRot", r.nextFloat() * 360.0F - 180.0F);
        put(state, "zBodyRot", r.nextFloat() * 360.0F - 180.0F);
        put(state, "attachFace", Direction.values()[r.nextInt(6)]);
        put(state, "invulnerableTicks", r.nextInt(3) == 0 ? r.nextFloat() * 220.0F : 0.0F);
        put(state, "isFallFlying", r.nextInt(3) == 0);
        put(state, "fallFlyingTimeInTicks", r.nextFloat() * 15.0F);
        put(state, "shouldApplyFlyingYRot", r.nextBoolean());
        put(state, "flyingYRot", r.nextFloat() * 2.0F - 1.0F);
        put(state, "isVisuallySwimming", r.nextBoolean());
        put(state, "fuseRemainingTicks", r.nextFloat() * 15.0F);
        put(state, "isInvisible", r.nextInt(4) == 0);
    }

    private static Field field(Class<?> type, String name) {
        for (Class<?> c = type; c != null; c = c.getSuperclass()) {
            try {
                var f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f;
            } catch (NoSuchFieldException e) {
            }
        }
        return null;
    }

    /** Sets `name` when the state has it, converting a float to an int field. */
    private static void put(Object state, String name, Object value) {
        var f = field(state.getClass(), name);
        if (f == null) return;
        try {
            if (f.getType() == int.class) f.setInt(state, ((Number) value).intValue());
            else f.set(state, value);
        } catch (ReflectiveOperationException | IllegalArgumentException mismatch) {
        }
    }

    private static Map<String, Object> read(Object state) throws Exception {
        var out = new TreeMap<String, Object>();
        for (String name : FIELDS) {
            var f = field(state.getClass(), name);
            if (f == null) continue;
            Object value = f.get(state);
            if (value instanceof Enum<?> e) value = e.name();
            if (value instanceof Number || value instanceof Boolean || value instanceof String) out.put(name, value);
        }
        return out;
    }

    private static Method find(Class<?> type, String name) {
        for (Class<?> c = type; c != null; c = c.getSuperclass())
            for (Method m : c.getDeclaredMethods())
                if (m.getName().equals(name) && !m.isBridge() && m.getParameterCount() == (name.equals("scale") ? 2 : 4)) {
                    m.setAccessible(true);
                    return m;
                }
        throw new IllegalArgumentException(type + " has no " + name);
    }
}
