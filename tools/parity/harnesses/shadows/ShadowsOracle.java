package acacia.parity.shadows;

import acacia.parity.Client;
import acacia.parity.Headless;
import acacia.parity.Out;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.file.Path;
import java.util.*;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.client.renderer.feature.ShadowFeatureRenderer;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.entity.EntityTypes;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.RenderShape;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.dimension.DimensionType;
import com.mojang.blaze3d.vertex.VertexConsumer;
import org.joml.Matrix4f;

/**
 * Entity shadows (docs/parity.md, harness shadows), args[0]/shadows.jsonl:
 * `radius|<type>|<adult|baby>`: every entity type's renderer's getShadowRadius and
 * getShadowStrength on the render state extracted from a new entity (babies where Mob.setBaby
 * makes one; cube mobs size 2, pufferfish puff 2), as `radius`, `strength`;
 * `pieces|<n>`: random ground and light under a shadow of random radius, height and camera
 * distance, through EntityRenderer.extractShadow on the real client level (blocks set there; the
 * raw brightness of each cell scripted and recorded in `cells` with whether the block below
 * is drawn with a full collision shape), the pieces it keeps and the vertices
 * ShadowFeatureRenderer.prepare makes of them (x, y, z, u, v, alpha) from an identity pose.
 */
public final class ShadowsOracle {
    /** The client level with each cell's raw brightness scripted; queries are recorded. */
    static final class Scripted extends ClientLevel {
        Scripted() {
            super(null, null, null, null, 0, 0, null, false, 0L, 0);
        }

        ClientLevel real;
        Random random;
        Map<String, Integer> asked;

        @Override
        public ChunkAccess getChunk(BlockPos pos) {
            return real.getChunk(pos);
        }

        @Override
        public DimensionType dimensionType() {
            return real.dimensionType();
        }

        @Override
        public int getMaxLocalRawBrightness(BlockPos pos) {
            return asked.computeIfAbsent(pos.getX() + "," + pos.getY() + "," + pos.getZ(), k -> random.nextInt(16));
        }
    }

    static final float[] RADII = {0.15F, 0.3F, 0.5F, 0.7F, 1.0F, 2.0F, 3.5F};

    public static void main(String[] args) throws Exception {
        var mc = Client.minecraft();
        BlockState[] GROUND = {
            Blocks.STONE.defaultBlockState(), Blocks.GRASS_BLOCK.defaultBlockState(), Blocks.GLASS.defaultBlockState(), Blocks.OAK_SLAB.defaultBlockState(),
            Blocks.BARRIER.defaultBlockState(), Blocks.MOSS_CARPET.defaultBlockState(), Blocks.DIRT_PATH.defaultBlockState(), Blocks.OAK_LEAVES.defaultBlockState(),
            Blocks.AIR.defaultBlockState(), Blocks.AIR.defaultBlockState(),
        };
        var dispatcher = Client.entityRenderers();
        var level = Client.overworld();
        // Client.level's connection skips its field initialisers; the wither's name asks the scoreboard.
        Headless.set(Headless.get(level, "connection"), "scoreboard", new net.minecraft.world.scores.Scoreboard());
        try (var out = new Out.Lines(Path.of(args[0]).resolve("shadows.jsonl"))) {
            radii(dispatcher, level, out);
            var pig = dispatcher.getRenderer(Client.spawn(level, EntityTypes.PIG,0.5, 70, 0.5));
            var scripted = Headless.allocate(Scripted.class);
            scripted.real = level;
            Method extract = EntityRenderer.class.getDeclaredMethod("extractShadow", EntityRenderState.class, Minecraft.class, Level.class);
            extract.setAccessible(true);
            Method prepare = ShadowFeatureRenderer.class.getDeclaredMethod("prepare", ShadowFeatureRenderer.Submit.class, VertexConsumer.class);
            prepare.setAccessible(true);
            var feature = new ShadowFeatureRenderer();
            var random = new Random(26);
            for (int n = 0; n < 1500; n++) {
                for (int x = -4; x < 5; x++) for (int z = -4; z < 5; z++) for (int y = 60; y < 70; y++) {
                    Client.setBlock(level, new BlockPos(x, y, z), y < 62 ? GROUND[0] : y > 66 ? Blocks.AIR.defaultBlockState() : GROUND[random.nextInt(GROUND.length)]);
                }
                var state = (LivingEntityRenderState) Headless.call(EntityRenderer.class, pig, "createRenderState", new Class<?>[0]);
                state.x = random.nextDouble() * 4 - 2;
                state.y = 62 + random.nextDouble() * 5;
                state.z = random.nextDouble() * 4 - 2;
                state.distanceToCameraSq = random.nextDouble() * 300;
                float radius = RADII[random.nextInt(RADII.length)];
                Headless.set(pig, "shadowRadius", radius);
                Headless.set(pig, "shadowStrength", random.nextInt(4) == 0 ? 0.5F : 1.0F);
                scripted.random = new Random(n);
                scripted.asked = new TreeMap<>();
                extract.invoke(pig, state, mc, scripted);
                var cells = new TreeMap<String, Object>();
                for (var e : scripted.asked.entrySet()) {
                    var p = e.getKey().split(",");
                    var below = new BlockPos(Integer.parseInt(p[0]), Integer.parseInt(p[1]) - 1, Integer.parseInt(p[2]));
                    var b = level.getBlockState(below);
                    boolean floor = b.getRenderShape() != RenderShape.INVISIBLE && b.isCollisionShapeFullBlock(level, below);
                    cells.put(e.getKey(), Out.map("brightness", e.getValue(), "floor", floor));
                }
                var pieces = new ArrayList<Object>();
                for (var piece : state.shadowPieces) {
                    var box = piece.shapeBelow().bounds();
                    pieces.add(Out.map("relative", new float[] {piece.relativeX(), piece.relativeY(), piece.relativeZ()}, "alpha", piece.alpha(),
                        "bounds", new double[] {box.minX, box.minY, box.minZ, box.maxX, box.maxY, box.maxZ}));
                }
                var vertices = new ArrayList<float[]>();
                VertexConsumer builder = (VertexConsumer) Proxy.newProxyInstance(VertexConsumer.class.getClassLoader(), new Class<?>[] {VertexConsumer.class}, (proxy, method, a) -> {
                    if (method.getName().equals("addVertex") && a != null && a.length == 11) {
                        int color = (Integer) a[3];
                        vertices.add(new float[] {(Float) a[0], (Float) a[1], (Float) a[2], (Float) a[4], (Float) a[5], (color >>> 24) / 255.0F});
                    }
                    var type = method.getReturnType();
                    if (type.isInstance(proxy)) return proxy;
                    return type == float.class ? 0.0F : type == int.class ? 0 : type == boolean.class ? false : null;
                });
                prepare.invoke(feature, new ShadowFeatureRenderer.Submit(new Matrix4f(), state.shadowRadius, List.copyOf(state.shadowPieces)), builder);
                out.write(Out.map("case", "pieces|" + n, "x", state.x, "y", state.y, "z", state.z, "distance_sq", state.distanceToCameraSq,
                    "radius", radius, "strength", Headless.get(pig, "shadowStrength"), "ambient", level.dimensionType().ambientLight(),
                    "cells", cells, "pieces", pieces, "vertices", vertices));
            }
        }
    }

    private static void radii(net.minecraft.client.renderer.entity.EntityRenderDispatcher dispatcher, ClientLevel level, Out.Lines out) throws Exception {
        for (var type : BuiltInRegistries.ENTITY_TYPE) {
            var id = BuiltInRegistries.ENTITY_TYPE.getKey(type).toString();
            for (boolean baby : new boolean[] {false, true}) {
                var record = new TreeMap<String, Object>();
                record.put("case", "radius|" + id + "|" + (baby ? "baby" : "adult"));
                try {
                    var entity = type.create(level, net.minecraft.world.entity.EntitySpawnReason.LOAD);
                    // Mannequins are Java's alone (no Bedrock entity) and need a skin to render.
                    if (entity == null || id.equals("minecraft:mannequin")) continue;
                    entity.setId(1_000_000);
                    if (baby && entity instanceof net.minecraft.world.entity.Mob mob) {
                        try {
                            mob.setBaby(true);
                        } catch (RuntimeException noBaby) {
                            // The wither's asks the scoreboard, which the bare level has not.
                            continue;
                        }
                    }
                    if (baby && !(entity instanceof net.minecraft.world.entity.LivingEntity l && l.isBaby())) continue;
                    for (var setter : new Object[][] {{"setSize", new Class<?>[] {int.class, boolean.class}, new Object[] {2, false}}, {"setPuffState", new Class<?>[] {int.class}, new Object[] {2}}}) {
                        try {
                            entity.getClass().getMethod((String) setter[0], (Class<?>[]) setter[1]).invoke(entity, (Object[]) setter[2]);
                        } catch (NoSuchMethodException absent) {
                        }
                    }
                    entity.refreshDimensions();
                    var renderer = dispatcher.getRenderer(entity);
                    EntityRenderState state;
                    try {
                        state = ((EntityRenderer<net.minecraft.world.entity.Entity, ?>) renderer).createRenderState(entity, 1.0F);
                    } catch (RuntimeException unextractable) {
                        // Withers ask the scoreboard, which the bare level has not: a fresh state.
                        state = (EntityRenderState) Headless.call(EntityRenderer.class, renderer, "createRenderState", new Class<?>[0]);
                        record.put("fresh", true);
                    }
                    record.put("radius", Headless.call(EntityRenderer.class, renderer, "getShadowRadius", new Class<?>[] {EntityRenderState.class}, state));
                    record.put("strength", Headless.call(EntityRenderer.class, renderer, "getShadowStrength", new Class<?>[] {EntityRenderState.class}, state));
                } catch (Throwable error) {
                    var cause = error.getCause() != null ? error.getCause() : error;
                    record.put("error", cause + " at " + java.util.Arrays.toString(java.util.Arrays.copyOf(cause.getStackTrace(), 4)));
                }
                out.write(record);
            }
        }
    }
}
