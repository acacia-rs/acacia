package acacia.parity.fluids;

import acacia.parity.Game;
import acacia.parity.Out;
import acacia.parity.UnitAtlas;
import com.mojang.blaze3d.vertex.VertexConsumer;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Set;
import net.minecraft.client.renderer.block.FluidRenderer;
import net.minecraft.client.renderer.block.FluidStateModelSet;
import net.minecraft.client.renderer.chunk.ChunkSectionLayer;
import net.minecraft.client.renderer.texture.SpriteContents;
import net.minecraft.client.renderer.texture.SpriteLoader;
import net.minecraft.client.renderer.texture.TextureAtlas;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.resources.model.sprite.Material;
import net.minecraft.client.resources.model.sprite.MaterialBaker;
import net.minecraft.client.resources.model.ModelDebugName;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.AtlasIds;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.VanillaPackResourcesBuilder;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagKey;
import net.minecraft.tags.TagLoader;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.LiquidBlock;

/**
 * FluidRenderer.tesselate over every liquid cell of each case in {@link Cases}. Writes
 * `cases.jsonl` (the blocks, as Java state keys) and `quads.jsonl` (the four vertices of each quad
 * the renderer emits: section-local position, sprite-local UV, ARGB colour, packed light).
 *
 * Fluid sprites sit side by side in one strip with a gap after each, so a vertex's u says which
 * sprite it samples (after Horizon's VanillaSectionMeshOracle).
 */
public final class FluidsOracle {
    static final String[] SPRITES = {"minecraft:block/water_still", "minecraft:block/water_flow",
        "minecraft:block/water_overlay", "minecraft:block/lava_still", "minecraft:block/lava_flow"};
    static final int STRIP = 2 * SPRITES.length;

    private FluidsOracle() {}

    private static final class Sprite extends TextureAtlasSprite {
        Sprite(SpriteContents contents, int slot) {
            super(TextureAtlas.LOCATION_BLOCKS, contents, contents.width() * STRIP, contents.height(), contents.width() * 2 * slot, 0, 0);
        }
    }

    /** The client's own sprites (their transparency picks the fluid's layer), each moved into its strip slot. */
    private static final class Materials extends MaterialBaker {
        final SpriteLoader.Preparations atlas;

        Materials(SpriteLoader.Preparations atlas) {
            super(atlas, atlas);
            this.atlas = atlas;
        }

        @Override
        public Material.Baked get(Material material, ModelDebugName name) {
            int slot = Arrays.asList(SPRITES).indexOf(material.sprite().toString());
            var sprite = atlas.getSprite(material.sprite());
            if (slot < 0 || sprite == null) throw new AssertionError("unexpected fluid sprite " + material.sprite());
            return new Material.Baked(new Sprite(sprite.contents(), slot), material.forceTranslucent());
        }
    }

    /** Binds the jar's block tags (FlowingFluid.getFlow reads #blocks_fluid_flow). */
    private static void bindBlockTags() {
        var info = new net.minecraft.server.packs.PackLocationInfo(net.minecraft.server.packs.repository.BuiltInPackSource.VANILLA_ID,
            net.minecraft.network.chat.Component.literal("vanilla"), net.minecraft.server.packs.repository.PackSource.BUILT_IN,
            java.util.Optional.of(net.minecraft.server.packs.repository.BuiltInPackSource.CORE_PACK_INFO));
        var pack = new VanillaPackResourcesBuilder().exposeNamespace("minecraft").pushJarResources().build(info);
        var data = new MultiPackResourceManager(PackType.SERVER_DATA, List.of(pack.fullResources()));
        @SuppressWarnings("unchecked")
        var lookup = (TagLoader.ElementLookup<Holder<Block>>) TagLoader.ElementLookup.fromFrozenRegistry(BuiltInRegistries.BLOCK);
        var loader = new TagLoader<>(lookup, Registries.tagsDirPath(Registries.BLOCK));
        var tags = new java.util.HashMap<TagKey<Block>, List<Holder<Block>>>();
        loader.build(loader.load(data)).forEach((id, values) -> tags.put(TagKey.create(Registries.BLOCK, id), List.copyOf(values)));
        if (tags.isEmpty()) throw new AssertionError("no block tags in the jar");
        BuiltInRegistries.BLOCK.prepareTagReload(new TagLoader.LoadResult<>(Registries.BLOCK, tags)).apply();
    }

    private static final class Recorder implements VertexConsumer {
        final List<Object> quads = new ArrayList<>();
        List<Object> pending = new ArrayList<>();
        int[] block;
        String layer;

        @Override
        public void addVertex(float x, float y, float z, int color, float u, float v, int overlay, int light, float nx, float ny, float nz) {
            int slot = (int) Math.floor(u * STRIP / 2.0f);
            pending.add(new Object[]{x, y, z, u * STRIP - 2 * slot, v, color & 0xFFFFFF, color >>> 24, light, SPRITES[slot]});
            if (pending.size() == 4) {
                quads.add(Out.map("block", block, "layer", layer, "vertices", pending));
                pending = new ArrayList<>();
            }
        }

        public VertexConsumer addVertex(float x, float y, float z) { throw new AssertionError(); }
        public VertexConsumer setColor(int r, int g, int b, int a) { throw new AssertionError(); }
        public VertexConsumer setColor(int color) { throw new AssertionError(); }
        public VertexConsumer setUv(float u, float v) { throw new AssertionError(); }
        public VertexConsumer setUv1(int u, int v) { throw new AssertionError(); }
        public VertexConsumer setUv2(int u, int v) { throw new AssertionError(); }
        public VertexConsumer setUv3(float u, float v) { throw new AssertionError(); }
        public VertexConsumer setNormal(float x, float y, float z) { throw new AssertionError(); }
        public VertexConsumer setLineWidth(float width) { throw new AssertionError(); }
    }

    private static List<Object> tesselate(Scene scene, FluidRenderer renderer) {
        var recorder = new Recorder();
        for (var pos : BlockPos.betweenClosed(0, Scene.MIN_Y, 0, 15, Scene.MIN_Y + 15, 15)) {
            var state = scene.getBlockState(pos);
            var fluid = state.getFluidState();
            if (fluid.isEmpty()) continue;
            recorder.block = new int[]{pos.getX(), pos.getY(), pos.getZ()};
            renderer.tesselate(scene, pos.immutable(), (ChunkSectionLayer layer) -> {
                recorder.layer = layer.name().toLowerCase(java.util.Locale.ROOT);
                return recorder;
            }, state, fluid);
        }
        if (!recorder.pending.isEmpty()) throw new AssertionError("partial quad");
        return recorder.quads;
    }

    /** `[x, y, z, state, liquid]`, `liquid` the state of a waterlogged block's water (null for none or a liquid block). */
    private static List<Object> blocks(Scene scene) {
        var out = new ArrayList<Object>();
        scene.blocks.forEach((key, state) -> {
            var p = BlockPos.of(key);
            var fluid = state.getFluidState();
            var liquid = fluid.isEmpty() || state.getBlock() instanceof LiquidBlock ? null : Out.stateKey(fluid.createLegacyBlock());
            out.add(Arrays.asList(p.getX(), p.getY(), p.getZ(), Out.stateKey(state), liquid));
        });
        return out;
    }

    public static void main(String[] args) throws Exception {
        var out = Path.of(args[0]);
        Game.init();
        bindBlockTags();
        var models = FluidStateModelSet.bake(new Materials(UnitAtlas.load(AtlasIds.BLOCKS, TextureAtlas.LOCATION_BLOCKS, Set.of(), false)));
        var renderer = new FluidRenderer(new FluidStateModelSet(models, models.values().iterator().next()));
        int count = 0;
        try (var cases = new Out.Lines(out.resolve("cases.jsonl")); var quads = new Out.Lines(out.resolve("quads.jsonl"))) {
            for (var scene : Cases.all()) {
                cases.write(Out.map("case", scene.name, "biome", scene.biomeName, "nether", scene.nether, "blocks", blocks(scene)));
                quads.write(Out.map("case", scene.name, "quads", tesselate(scene, renderer)));
                count++;
            }
        }
        Facts.write(out.resolve("facts.jsonl"));
        System.out.println("fluids: " + count + " cases");
    }
}
