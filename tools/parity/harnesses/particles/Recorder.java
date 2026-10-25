package acacia.parity.particles;

import acacia.parity.Shell;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Queue;
import java.util.concurrent.atomic.AtomicLong;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.particle.NoRenderParticle;
import net.minecraft.client.particle.Particle;
import net.minecraft.client.particle.ParticleEngine;
import net.minecraft.client.particle.ParticleResources;
import net.minecraft.client.particle.SingleQuadParticle;
import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.levelgen.RandomSupport;

/**
 * The real ParticleEngine, logging every createParticle (type, position, velocity) by tick and
 * naming each particle by its own random's seed. Particle.random is RandomSource.create(), seeded
 * from SEED_UNIQUIFIER ^ System.nanoTime(): stepping the random's state back to where some
 * uniquifier value XOR a nanoTime inside the call's window explains it recovers the seed (the
 * probe replays it), as Horizon's provider oracle does.
 */
final class Recorder extends ParticleEngine {
    private static final long MULTIPLIER = 0x5DEECE66DL, MASK = (1L << 48) - 1, INVERSE = 0xDFE05BCB1365L;
    private static final long UNIQUIFIER_STEP = 1181783497276652981L;
    private static final AtomicLong UNIQUIFIER = Shell.get(RandomSupport.class, "SEED_UNIQUIFIER");

    int tick;
    final List<List<Object>> spawns = new ArrayList<>();
    final List<List<Object>> seeds = new ArrayList<>();
    private final Map<Particle, Long> seedOf = new IdentityHashMap<>();
    private final Map<Particle, String> typeOf = new IdentityHashMap<>();

    Recorder(ClientLevel level, ParticleResources resources, RandomSource random) {
        super(level, resources);
        Shell.set(this, "random", random);
    }

    static String name(ParticleOptions o) {
        return BuiltInRegistries.PARTICLE_TYPE.getKey(o.getType()).getPath();
    }

    @Override
    public Particle createParticle(ParticleOptions o, double x, double y, double z, double xa, double ya, double za) {
        var type = name(o);
        spawns.add(List.of(type, x, y, z, xa, ya, za));
        long uniquifier = UNIQUIFIER.get(), t0 = System.nanoTime();
        var p = super.createParticle(o, x, y, z, xa, ya, za);
        long t1 = System.nanoTime();
        if (p != null) {
            long seed = recover(p, uniquifier, t0, t1);
            seedOf.put(p, seed);
            typeOf.put(p, type);
            seeds.add(List.of(tick, type, x, y, z, seed));
        }
        return p;
    }

    /** The seed whose `new Random(seed)` state the particle's random started from (low 48 bits). */
    private static long recover(Particle p, long uniquifier, long t0, long t1) {
        AtomicLong state = Shell.get(Shell.get(p, "random"), "seed");
        long lo = t0 & MASK, hi = t1 & MASK, u = uniquifier;
        for (int creates = 0; creates < 8; creates++) {
            u *= UNIQUIFIER_STEP;
            long s = state.get();
            for (int draws = 0; draws < 512; draws++) {
                long nano = (s ^ MULTIPLIER ^ u) & MASK;
                if (lo <= hi ? nano >= lo && nano <= hi : nano >= lo || nano <= hi) return (s ^ MULTIPLIER) & MASK;
                s = ((s - 11) * INVERSE) & MASK;
            }
        }
        throw new IllegalStateException("no seed explains " + p.getClass().getSimpleName() + "'s random");
    }

    /** The spawns since the last call, sorted by type then numbers. */
    List<List<Object>> takeSpawns() {
        var out = new ArrayList<>(spawns);
        spawns.clear();
        Comparator<List<Object>> byType = Comparator.comparing(s -> (String) s.get(0));
        for (int i = 1; i < 7; i++) {
            int k = i;
            byType = byType.thenComparing(s -> (Double) s.get(k), Double::compare);
        }
        out.sort(byType);
        return out;
    }

    /** Every live drawn particle after the tick, by seed. */
    List<Map<String, Object>> snapshot() {
        Map<?, ?> groups = Shell.get(this, "particles");
        var out = new ArrayList<Map<String, Object>>();
        for (Object group : groups.values()) {
            Queue<?> particles = Shell.get(group, "particles");
            for (Object o : particles) {
                if (o instanceof NoRenderParticle || !(o instanceof SingleQuadParticle p)) continue;
                out.add(acacia.parity.Out.map(
                    "seed", seedOf.getOrDefault(p, 0L),
                    "type", typeOf.getOrDefault(p, "?"),
                    "pos", List.of(d(p, "x"), d(p, "y"), d(p, "z")),
                    "vel", List.of(d(p, "xd"), d(p, "yd"), d(p, "zd")),
                    "age", Shell.get(p, "age"),
                    "lifetime", Shell.get(p, "lifetime"),
                    "sprite", sprite(p),
                    "size", (double) p.getQuadSize(0.0F),
                    "rgba", List.of(f(p, "rCol"), f(p, "gCol"), f(p, "bCol"), f(p, "alpha")),
                    "ground", Shell.get(p, "onGround")));
            }
        }
        out.sort(Comparator.comparingLong(m -> (Long) m.get("seed")));
        return out;
    }

    private static double d(Object p, String field) {
        return Shell.<Double>get(p, field);
    }

    private static double f(Object p, String field) {
        return Shell.<Float>get(p, field);
    }

    private static String sprite(SingleQuadParticle p) {
        net.minecraft.client.renderer.texture.TextureAtlasSprite s = Shell.get(p, "sprite");
        return s == null ? null : s.contents().name().getPath();
    }
}
