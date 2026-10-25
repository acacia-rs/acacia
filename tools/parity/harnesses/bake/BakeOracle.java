package acacia.parity.bake;

import acacia.parity.BlockModels;
import acacia.parity.Game;
import acacia.parity.Out;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Path;
import java.util.*;
import net.minecraft.client.color.block.BlockTintSource;
import net.minecraft.client.renderer.BiomeColors;
import net.minecraft.client.renderer.block.BlockAndTintGetter;
import net.minecraft.client.renderer.block.dispatch.BlockStateModel;
import net.minecraft.client.renderer.block.dispatch.BlockStateModelPart;
import net.minecraft.client.renderer.block.dispatch.SingleVariant;
import net.minecraft.client.renderer.block.dispatch.WeightedVariants;
import net.minecraft.client.renderer.block.dispatch.multipart.MultiPartModel;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.util.RandomSource;
import net.minecraft.util.random.Weighted;
import net.minecraft.util.random.WeightedList;
import net.minecraft.world.level.ColorResolver;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;

/**
 * Dumps what `lookbake java-check` compares (docs/parity.md, harness bake) into args[0]:
 * quads.json.gz (every block state's baked parts and their alternatives), placement.json (the
 * alternatives picked and the offsets at sample positions), biomes.json, shade.json, colors.json.
 */
public final class BakeOracle {
    private static final Direction[] SIDES = {null, Direction.DOWN, Direction.UP, Direction.NORTH, Direction.SOUTH, Direction.WEST, Direction.EAST};

    private final List<Object> parts = new ArrayList<>();
    private final Map<String, Integer> partIndex = new HashMap<>();
    private final IdentityHashMap<BlockStateModelPart, Integer> partOf = new IdentityHashMap<>();

    public static void main(String[] args) throws Exception {
        var out = Path.of(args[0]);
        var models = BlockModels.get();
        new BakeOracle().dump(models, out);
        biomes(out.resolve("biomes.json"));
        shadeAndColors(out);
    }

    // ---- quads and placement ----

    private void dump(BlockModels models, Path out) throws Exception {
        var positions = positions();
        Map<String, Object> states = new TreeMap<>();
        Map<String, Object> picks = new TreeMap<>();
        int alternatives = 0;
        for (BlockState state : Block.BLOCK_STATE_REGISTRY) {
            var key = Out.stateKey(state);
            var model = models.model(state);
            if (model == null) {
                states.put(key, Out.map("parts", List.of(), "slots", null));
                continue;
            }
            List<List<Object[]>> slots = slots(model, state);
            boolean varies = slots.stream().anyMatch(s -> s.size() > 1);
            var first = slots.stream().map(s -> (int) s.getFirst()[1]).toList();
            Object slotJson = null;
            if (varies) {
                alternatives++;
                slotJson = slots.stream().map(s -> s.stream().map(a -> Out.map("weight", a[0], "part", a[1])).toList()).toList();
                var random = RandomSource.createThreadLocalInstance(0L);
                var picked = new ArrayList<Object>();
                var collected = new ArrayList<BlockStateModelPart>();
                for (var p : positions) {
                    random.setSeed(state.getSeed(p));
                    collected.clear();
                    model.collectParts(random, collected);
                    picked.add(collected.stream().map(this::index).toList());
                }
                picks.put(key, picked);
            }
            states.put(key, Out.map("parts", first, "slots", slotJson));
        }
        Out.json(out.resolve("quads.json.gz"), Out.map("parts", parts, "states", states));
        Map<String, Object> offsets = new TreeMap<>();
        for (Block block : BuiltInRegistries.BLOCK) {
            var state = block.defaultBlockState();
            if (!state.hasOffsetFunction()) continue;
            offsets.put(BuiltInRegistries.BLOCK.getKey(block).toString(), positions.stream().map(p -> {
                var o = state.getOffset(p);
                return new double[]{o.x, o.y, o.z};
            }).toList());
        }
        Out.json(out.resolve("placement.json"), Out.map(
            "positions", positions.stream().map(p -> new int[]{p.getX(), p.getY(), p.getZ()}).toList(),
            "offsets", offsets, "picks", picks));
        System.out.printf("%d states, %d with alternatives, %d distinct parts%n", states.size(), alternatives, parts.size());
    }

    /** 48 positions: the origin and its neighbours, then spread ones of both signs. */
    private static List<BlockPos> positions() {
        var out = new ArrayList<BlockPos>(List.of(BlockPos.ZERO, new BlockPos(1, 0, 0), new BlockPos(0, 1, 0), new BlockPos(0, 0, 1),
            new BlockPos(-1, 64, -1), new BlockPos(15, 15, 15), new BlockPos(16, -64, 16), new BlockPos(-16, 319, -16)));
        var random = new Random(0x6a617661L);
        while (out.size() < 48) out.add(new BlockPos(random.nextInt(-30_000_000, 30_000_000), random.nextInt(-64, 320), random.nextInt(-30_000_000, 30_000_000)));
        return out;
    }

    /** One slot per model drawn, each its alternatives as {weight, part index}. */
    private List<List<Object[]>> slots(BlockStateModel model, BlockState state) throws Exception {
        if (model instanceof MultiPartModel multipart) {
            var shared = field(MultiPartModel.class, "shared").get(multipart);
            Method select = shared.getClass().getDeclaredMethod("selectModels", BlockState.class);
            select.setAccessible(true);
            var out = new ArrayList<List<Object[]>>();
            for (var selected : (List<?>) select.invoke(shared, state)) out.add(slot((BlockStateModel) selected));
            return out;
        }
        return List.of(slot(model));
    }

    private List<Object[]> slot(BlockStateModel model) throws Exception {
        if (model instanceof SingleVariant single) {
            return List.<Object[]>of(new Object[]{1, index((BlockStateModelPart) field(SingleVariant.class, "model").get(single))});
        }
        if (model instanceof WeightedVariants weighted) {
            var out = new ArrayList<Object[]>();
            for (var entry : ((WeightedList<?>) field(WeightedVariants.class, "list").get(weighted)).unwrap()) {
                var alternative = slot((BlockStateModel) ((Weighted<?>) entry).value());
                if (alternative.size() != 1) throw new IllegalStateException("nested weighted variants");
                out.add(new Object[]{((Weighted<?>) entry).weight(), alternative.getFirst()[1]});
            }
            return out;
        }
        throw new IllegalStateException("unknown block state model " + model.getClass());
    }

    private int index(BlockStateModelPart part) {
        return partOf.computeIfAbsent(part, p -> {
            var json = describe(p);
            return partIndex.computeIfAbsent(Out.JSON.toJson(json), k -> {
                parts.add(json);
                return parts.size() - 1;
            });
        });
    }

    private static Object describe(BlockStateModelPart part) {
        var quads = new ArrayList<Object>();
        for (var side : SIDES) {
            for (BakedQuad quad : part.getQuads(side)) {
                var info = quad.materialInfo();
                var positions = new float[4][];
                var uvs = new float[4][];
                for (int i = 0; i < 4; i++) {
                    var p = quad.position(i);
                    positions[i] = new float[]{p.x(), p.y(), p.z()};
                    long uv = quad.packedUV(i);
                    uvs[i] = new float[]{Float.intBitsToFloat((int) (uv >>> 32)) * 16, Float.intBitsToFloat((int) uv) * 16};
                }
                var override = info.shadeDirectionOverride();
                quads.add(Out.map("positions", positions, "uvs", uvs, "sprite", info.sprite().contents().name().toString(),
                    "tint_index", info.tintIndex(), "direction", quad.direction().getName(),
                    "shade_direction_override", override == null ? null : override.getName(),
                    "cullface", side == null ? null : side.getName()));
            }
        }
        return Out.map("ambient_occlusion", part.useAmbientOcclusion(), "quads", quads);
    }

    private static Field field(Class<?> owner, String name) throws Exception {
        Field f = owner.getDeclaredField(name);
        f.setAccessible(true);
        return f;
    }

    // ---- biomes, shade, colours ----

    private static void biomes(Path path) throws Exception {
        var random = new Random(0x62696f6dL);
        var columns = new ArrayList<int[]>();
        for (int i = 0; i < 64; i++) columns.add(new int[]{random.nextInt(-4096, 4096), random.nextInt(-4096, 4096)});
        Map<String, Object> biomes = new TreeMap<>();
        Game.world().lookupOrThrow(Registries.BIOME).listElements().forEach(holder -> {
            var biome = holder.value();
            biomes.put(holder.key().identifier().toString(), Out.map(
                "foliage", Out.rgb(biome.getFoliageColor()), "dry_foliage", Out.rgb(biome.getDryFoliageColor()),
                "water", Out.rgb(biome.getWaterColor()),
                "grass", columns.stream().map(c -> Out.rgb(biome.getGrassColor(c[0], c[1]))).toList()));
        });
        Out.json(path, Out.map("columns", columns, "biomes", biomes));
        System.out.printf("%d biomes%n", biomes.size());
    }

    private static final Map<ColorResolver, String> RESOLVERS = Map.of(BiomeColors.GRASS_COLOR_RESOLVER, "grass",
        BiomeColors.FOLIAGE_COLOR_RESOLVER, "foliage", BiomeColors.DRY_FOLIAGE_COLOR_RESOLVER, "dry_foliage", BiomeColors.WATER_COLOR_RESOLVER, "water");
    private static final List<ColorResolver> ORDER = List.copyOf(RESOLVERS.keySet());

    /** Answers each biome colour with a sentinel so colours.json can name the resolver a layer reads. */
    private static BlockAndTintGetter sentinels(BlockState at) {
        return new BlockAndTintGetter() {
            public net.minecraft.world.level.CardinalLighting cardinalLighting() { return EMPTY.cardinalLighting(); }
            public net.minecraft.world.level.lighting.LevelLightEngine getLightEngine() { return EMPTY.getLightEngine(); }
            public int getBlockTint(BlockPos pos, ColorResolver resolver) { return 0xFF5E0000 | ORDER.indexOf(resolver); }
            public net.minecraft.world.level.block.entity.BlockEntity getBlockEntity(BlockPos pos) { return null; }
            public BlockState getBlockState(BlockPos pos) { return pos.equals(BlockPos.ZERO) ? at : EMPTY.getBlockState(pos); }
            public net.minecraft.world.level.material.FluidState getFluidState(BlockPos pos) { return getBlockState(pos).getFluidState(); }
            public int getHeight() { return 384; }
            public int getMinY() { return -64; }
        };
    }

    private static Object color(int argb) {
        int resolver = argb & 0xFFFF;
        return (argb & 0xFFFF0000) == 0xFF5E0000 && resolver < ORDER.size() ? RESOLVERS.get(ORDER.get(resolver)) : Out.argb(argb);
    }

    private static void shadeAndColors(Path out) throws Exception {
        var dark = new TreeSet<String>();
        Map<String, Object> colors = new TreeMap<>();
        for (BlockState state : Block.BLOCK_STATE_REGISTRY) {
            var key = Out.stateKey(state);
            if (state.getShadeBrightness(EmptyBlockGetter.INSTANCE, BlockPos.ZERO) < 1.0f) dark.add(key);
            List<BlockTintSource> layers = Game.blockColors().getTintSources(state);
            if (layers.isEmpty()) continue;
            var level = sentinels(state);
            colors.put(key, Out.map(
                "in_hand", layers.stream().map(l -> l == null ? null : color(l.color(state))).toList(),
                "in_world", layers.stream().map(l -> l == null ? null : color(l.colorInWorld(state, level, BlockPos.ZERO))).toList()));
        }
        Out.json(out.resolve("shade.json"), Out.map("dark", dark));
        Out.json(out.resolve("colors.json"), Out.map("states", colors));
        System.out.printf("%d states darken ambient occlusion, %d tinted%n", dark.size(), colors.size());
    }
}
