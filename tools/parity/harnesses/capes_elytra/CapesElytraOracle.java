package acacia.parity.capes_elytra;

import acacia.parity.Client;
import acacia.parity.Headless;
import acacia.parity.Out;
import java.nio.file.Path;
import java.util.*;
import net.minecraft.client.entity.ClientAvatarState;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.client.player.RemotePlayer;
import net.minecraft.client.renderer.entity.player.AvatarRenderer;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import net.minecraft.world.entity.Avatar;
import net.minecraft.world.entity.ElytraAnimationState;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityTypes;
import net.minecraft.world.entity.Pose;
import net.minecraft.world.phys.Vec3;

/**
 * Capes and elytra over time (docs/parity.md, harness capes_elytra), args[0]/capes_elytra.jsonl:
 * `cape|<n>`: a player walked through random moves (steps, jumps, falls, teleports, standing),
 * its ClientAvatarState ticked (tick, updateBob, addWalkDistance) and after each tick, at a
 * random partial tick and body yaw, AvatarRenderer.extractCapeState's capeFlap, capeLean and
 * capeLean2; `wings|<n>`: a mob's ElytraAnimationState ticked through random gliding (with its
 * movement), crouching and standing, and getRotX/Y/Z at a random partial tick after each.
 * Each record lists its ticks: inputs and outputs.
 */
public final class CapesElytraOracle {
    public static void main(String[] args) throws Exception {
        Client.minecraft();
        var level = Client.overworld();
        var dispatcher = Client.entityRenderers();
        @SuppressWarnings("unchecked")
        var renderers = (Map<?, AvatarRenderer<AbstractClientPlayer>>) Headless.get(dispatcher, "playerRenderers");
        var renderer = renderers.values().iterator().next();
        var extract = AvatarRenderer.class.getDeclaredMethod("extractCapeState", findAvatarlike(), AvatarRenderState.class, float.class);
        extract.setAccessible(true);
        var random = new Random(26);
        try (var out = new Out.Lines(Path.of(args[0]).resolve("capes_elytra.jsonl"))) {
            for (int n = 0; n < 300; n++) {
                var player = Headless.allocate(RemotePlayer.class);
                var avatar = new ClientAvatarState();
                Headless.set(player, "clientAvatarState", avatar);
                var at = new Vec3(random.nextInt(64) - 32 + random.nextDouble(), 64, random.nextInt(64) - 32);
                Headless.set(player, "position", at);
                var ticks = new ArrayList<Object>();
                for (int t = 0; t < 40; t++) {
                    var old = at;
                    int move = random.nextInt(10);
                    var step = switch (move) {
                        case 0, 1 -> Vec3.ZERO;
                        case 2 -> new Vec3(0, 0.42, 0);
                        case 3 -> new Vec3(0, -0.6, 0);
                        case 4 -> new Vec3(random.nextGaussian() * 15, 0, 0);
                        default -> new Vec3(random.nextGaussian() * 0.25, random.nextGaussian() * 0.05, random.nextGaussian() * 0.25);
                    };
                    at = at.add(step);
                    float tBob = random.nextInt(3) == 0 ? 0.0F : Math.min(0.1F, (float) step.horizontalDistance());
                    float walked = random.nextBoolean() ? (float) step.horizontalDistance() * 0.6F : 0.0F;
                    avatar.tick(at, step);
                    avatar.updateBob(tBob);
                    avatar.addWalkDistance(walked);
                    float partial = random.nextFloat();
                    float yaw = random.nextFloat() * 720 - 360;
                    float fallFlying = random.nextInt(4) == 0 ? random.nextFloat() * 12 : 0;
                    Headless.set(player, "xo", old.x);
                    Headless.set(player, "yo", old.y);
                    Headless.set(player, "zo", old.z);
                    Headless.set(player, "position", at);
                    Headless.set(player, "yBodyRot", yaw);
                    Headless.set(player, "yBodyRotO", yaw);
                    var state = new AvatarRenderState();
                    state.fallFlyingTimeInTicks = fallFlying;
                    extract.invoke(renderer, player, state, partial);
                    ticks.add(Out.map("at", new double[] {at.x, at.y, at.z}, "old", new double[] {old.x, old.y, old.z}, "bob", tBob, "walked", walked,
                        "partial", partial, "yaw", yaw, "fall_flying_scale", state.fallFlyingScale(),
                        "cape", new float[] {state.capeFlap, state.capeLean, state.capeLean2}));
                }
                out.write(Out.map("case", "cape|" + n, "ticks", ticks));
            }
            var mob = Client.spawn(level, EntityTypes.ZOMBIE, 0.5, 70, 0.5);
            for (int n = 0; n < 300; n++) {
                var wings = new ElytraAnimationState(mob);
                var ticks = new ArrayList<Object>();
                for (int t = 0; t < 30; t++) {
                    int what = random.nextInt(3);
                    var movement = new Vec3(random.nextGaussian(), random.nextGaussian() - (random.nextBoolean() ? 1 : 0), random.nextGaussian());
                    Headless.call(Entity.class, mob, "setSharedFlag", new Class<?>[] {int.class, boolean.class}, 7, what == 0);
                    mob.setPose(what == 1 ? Pose.CROUCHING : Pose.STANDING);
                    mob.setDeltaMovement(movement);
                    wings.tick();
                    float partial = random.nextFloat();
                    ticks.add(Out.map("gliding", what == 0, "crouching", what == 1, "movement", new double[] {movement.x, movement.y, movement.z},
                        "partial", partial, "rot", new float[] {wings.getRotX(partial), wings.getRotY(partial), wings.getRotZ(partial)}));
                }
                out.write(Out.map("case", "wings|" + n, "ticks", ticks));
            }
        }
    }

    private static Class<?> findAvatarlike() {
        for (var m : AvatarRenderer.class.getDeclaredMethods())
            if (m.getName().equals("extractCapeState")) return m.getParameterTypes()[0];
        throw new IllegalStateException("no extractCapeState");
    }
}
