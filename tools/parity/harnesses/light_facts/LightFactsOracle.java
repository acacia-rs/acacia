package acacia.parity.light_facts;

import acacia.parity.FluidRendering;
import acacia.parity.Game;
import acacia.parity.LightShapes;
import acacia.parity.Out;
import java.nio.file.Path;
import java.util.TreeMap;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.core.Direction;
import net.minecraft.tags.BlockTags;
import net.minecraft.world.phys.shapes.Shapes;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.HalfTransparentBlock;
import net.minecraft.world.level.block.IronBarsBlock;
import net.minecraft.world.level.block.MangroveRootsBlock;
import net.minecraft.world.level.block.PowderSnowBlock;
import net.minecraft.world.level.block.state.BlockState;

/**
 * Every block state's light facts into args[0]/facts.json, by state key: what the light engine and
 * BlockModelLighter read of it (emission, dampening, solid render, full collision, emissive
 * rendering, shade brightness below 1) and its face occlusion shapes where the engine looks at
 * them (LightShapes; absent otherwise); for faces drawn against neighbours (`Block.shouldRenderFace`),
 * its face occlusion shapes when it can occlude, and which `skipRendering` rule it keeps.
 */
public final class LightFactsOracle {
    public static void main(String[] args) throws Exception {
        Game.init();
        FluidRendering.bindBlockTags();
        var level = EmptyBlockGetter.INSTANCE;
        var out = new TreeMap<String, Object>();
        for (BlockState s : Block.BLOCK_STATE_REGISTRY) {
            var facts = Out.map("emission", s.getLightEmission(), "dampening", s.getLightDampening(), "solid_render", s.isSolidRender(),
                "full_collision", s.isCollisionShapeFullBlock(level, BlockPos.ZERO), "emissive", s.emissiveRendering(),
                "dark", s.getShadeBrightness(level, BlockPos.ZERO) < 1.0F);
            var faces = LightShapes.of(s);
            if (faces != null) facts.put("faces", faces);
            if (s.canOcclude()) {
                facts.put("occlusion", LightShapes.faces(s));
                var whole = new java.util.ArrayList<Boolean>();
                for (var d : Direction.values()) whole.add(s.getFaceOcclusionShape(d) == Shapes.block());
                facts.put("occlusion_whole", whole);
            }
            var skip = skip(s);
            if (skip != null) facts.put("skip", skip);
            out.put(Out.stateKey(s), facts);
        }
        Out.json(Path.of(args[0], "facts.json"), out);
    }

    /** Which `skipRendering` rule the block keeps against its neighbours, by class. */
    private static String skip(BlockState s) {
        var block = s.getBlock();
        if (block instanceof IronBarsBlock) return s.is(BlockTags.BARS) ? "bars_tagged" : "bars";
        if (block instanceof HalfTransparentBlock || block instanceof PowderSnowBlock) return "same";
        if (block instanceof MangroveRootsBlock) return "same_vertical";
        return null;
    }
}
