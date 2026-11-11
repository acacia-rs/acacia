package acacia.parity.particles;

import acacia.parity.Client;
import acacia.parity.Out;
import acacia.parity.Shell;
import acacia.parity.View;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.particle.ParticleResources;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.particles.ColorParticleOption;
import net.minecraft.core.particles.DustColorTransitionOptions;
import net.minecraft.core.particles.DustParticleOptions;
import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.core.particles.ParticleType;
import net.minecraft.core.particles.SimpleParticleType;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.block.CampfireBlock;
import net.minecraft.world.level.block.entity.CampfireBlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.Vec3;

/**
 * Particles: each case on a fresh ClientLevel holding its blocks (unset cells air), with the
 * randoms seeded from the case's: animate `seed`, level `seed + 1`, ParticleEngine `seed + 3`.
 * A block case runs, each tick, a lit campfire's CampfireBlockEntity.particleTick, then
 * doAnimateTick's body at the cell (block and fluid animateTick, the drip below a fluid; no
 * ambient or marker particles), then ParticleEngine.tick. A particle case creates one particle
 * before the first tick, then ticks. Writes, per case: records.jsonl {case, ticks: [{spawns,
 * particles}]}, seeds.jsonl {case, seeds: [[tick, type, x, y, z, xa, ya, za, seed]]} for the probe to replay
 * the particles' own randoms, scenes.jsonl {case, blocks with full state keys, foliage}.
 *
 * args: <out dir> <cases.jsonl>
 */
public final class ParticlesOracle {
    private ParticlesOracle() {}

    public static void main(String[] args) throws Exception {
        var out = Path.of(args[0]);
        var mc = Client.minecraft();
        var resources = Sprites.resources();
        Shell.set(mc, "soundManager", Sprites.silent());
        try (var records = new Out.Lines(out.resolve("records.jsonl"));
             var seeds = new Out.Lines(out.resolve("seeds.jsonl"));
             var scenes = new Out.Lines(out.resolve("scenes.jsonl"))) {
            for (String line : Files.readAllLines(Path.of(args[1]))) {
                if (line.isBlank()) continue;
                var c = JsonParser.parseString(line).getAsJsonObject();
                var name = c.get("case").getAsString();
                try {
                    run(c, name, mc, resources, records, seeds, scenes);
                } catch (Exception e) {
                    e.printStackTrace();
                    scenes.write(Out.map("case", name, "error", e.toString()));
                }
            }
        }
    }

    private static void run(JsonObject c, String name, Minecraft mc, ParticleResources resources,
                            Out.Lines records, Out.Lines seeds, Out.Lines scenes) throws Exception {
        var level = Client.overworld();
        var placed = place(level, c.getAsJsonArray("blocks"));
        long seed = c.get("seed").getAsLong();
        int ticks = c.get("ticks").getAsInt();
        Shell.set(level, "random", RandomSource.create(seed + 1));
        var engine = new Recorder(level, resources, RandomSource.create(seed + 3));
        Shell.set(mc, "particleEngine", engine);
        var block = "block".equals(c.get("kind").getAsString());
        var cell = block ? pos(c.getAsJsonArray("cell")) : null;
        var at = block ? Vec3.atCenterOf(cell) : vec(c.getAsJsonObject("particle").getAsJsonArray("pos"));
        Shell.set(View.gameRenderer().mainCamera(), "position", at);
        var run = block ? block(level, engine, cell, RandomSource.create(seed), ticks) : particle(engine, c.getAsJsonObject("particle"), ticks);
        int foliage = block ? level.getClientLeafTintColor(cell) & 0xFFFFFF : 0;
        records.write(Out.map("case", name, "ticks", run));
        seeds.write(Out.map("case", name, "seeds", engine.seeds));
        scenes.write(Out.map("case", name, "blocks", placed, "foliage", foliage));
    }

    private static List<Object> place(ClientLevel level, JsonArray blocks) throws Exception {
        var placed = new ArrayList<Object>();
        for (var b : blocks) {
            var a = b.getAsJsonArray();
            var p = new BlockPos(a.get(0).getAsInt(), a.get(1).getAsInt(), a.get(2).getAsInt());
            BlockState state = BlockStateParser.parseForBlock(BuiltInRegistries.BLOCK, a.get(3).getAsString(), false).blockState();
            // Into the section: level.setBlock would make block entities, whose renderers this client lacks.
            var chunk = Client.chunk(level, p.getX() >> 4, p.getZ() >> 4);
            chunk.getSection(chunk.getSectionIndex(p.getY())).setBlockState(p.getX() & 15, p.getY() & 15, p.getZ() & 15, state);
            placed.add(List.of(p.getX(), p.getY(), p.getZ(), Out.stateKey(state)));
        }
        return placed;
    }

    private static List<Object> block(ClientLevel level, Recorder engine, BlockPos pos, RandomSource animate, int ticks) {
        var state = level.getBlockState(pos);
        var campfire = state.getBlock() instanceof CampfireBlock && state.getValue(CampfireBlock.LIT) ? new CampfireBlockEntity(pos, state) : null;
        var run = new ArrayList<Object>();
        for (int t = 0; t < ticks; t++) {
            engine.tick = t;
            if (campfire != null) CampfireBlockEntity.particleTick(level, pos, state, campfire);
            animateCell(level, pos, animate);
            engine.tick();
            run.add(Out.map("spawns", engine.takeSpawns(), "particles", engine.snapshot()));
        }
        return run;
    }

    /** ClientLevel.doAnimateTick after it has picked `pos`, without ambient and marker particles. */
    private static void animateCell(ClientLevel level, BlockPos pos, RandomSource animate) {
        var state = level.getBlockState(pos);
        state.getBlock().animateTick(state, level, pos, animate);
        var fluid = level.getFluidState(pos);
        if (fluid.isEmpty()) return;
        fluid.animateTick(level, pos, animate);
        var drip = fluid.getDripParticle();
        if (drip != null && level.getRandom().nextInt(10) == 0) {
            boolean watertight = state.isFaceSturdy(level, pos, Direction.DOWN);
            var below = pos.below();
            Shell.call(ClientLevel.class, level, "trySpawnDripParticles", new Class<?>[]{BlockPos.class, BlockState.class, ParticleOptions.class, boolean.class},
                below, level.getBlockState(below), drip, watertight);
        }
    }

    private static List<Object> particle(Recorder engine, JsonObject p, int ticks) {
        var options = options(p);
        var at = vec(p.getAsJsonArray("pos"));
        var v = vec(p.getAsJsonArray("vel"));
        var run = new ArrayList<Object>();
        for (int t = 0; t < ticks; t++) {
            engine.tick = t;
            if (t == 0) engine.createParticle(options, at.x, at.y, at.z, v.x, v.y, v.z);
            engine.tick();
            run.add(Out.map("spawns", engine.takeSpawns(), "particles", engine.snapshot()));
        }
        return run;
    }

    @SuppressWarnings("unchecked")
    private static ParticleOptions options(JsonObject p) {
        ParticleType<?> type = BuiltInRegistries.PARTICLE_TYPE.getValue(Identifier.withDefaultNamespace(p.get("type").getAsString()));
        float scale = p.has("scale") ? p.get("scale").getAsFloat() : 1.0F;
        if (type instanceof SimpleParticleType simple) return simple;
        int color = p.get("color").getAsInt();
        return switch (p.get("type").getAsString()) {
            case "dust" -> new DustParticleOptions(color, scale);
            case "dust_color_transition" -> new DustColorTransitionOptions(color, p.get("to").getAsInt(), scale);
            default -> ColorParticleOption.create((ParticleType<ColorParticleOption>) type, 0xFF000000 | color);
        };
    }

    private static BlockPos pos(JsonArray a) {
        return new BlockPos(a.get(0).getAsInt(), a.get(1).getAsInt(), a.get(2).getAsInt());
    }

    private static Vec3 vec(JsonArray a) {
        return new Vec3(a.get(0).getAsDouble(), a.get(1).getAsDouble(), a.get(2).getAsDouble());
    }
}
