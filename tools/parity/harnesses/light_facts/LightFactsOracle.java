package acacia.parity.light_facts;

import acacia.parity.Game;
import acacia.parity.LightShapes;
import acacia.parity.Out;
import java.nio.file.Path;
import java.util.TreeMap;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;

/**
 * Every block state's light facts into args[0]/facts.json, by state key: what the light engine and
 * BlockModelLighter read of it (emission, dampening, solid render, full collision, emissive
 * rendering, shade brightness below 1) and its face occlusion shapes where the engine looks at
 * them (LightShapes; absent otherwise).
 */
public final class LightFactsOracle {
    public static void main(String[] args) throws Exception {
        Game.init();
        var level = EmptyBlockGetter.INSTANCE;
        var out = new TreeMap<String, Object>();
        for (BlockState s : Block.BLOCK_STATE_REGISTRY) {
            var facts = Out.map("emission", s.getLightEmission(), "dampening", s.getLightDampening(), "solid_render", s.isSolidRender(),
                "full_collision", s.isCollisionShapeFullBlock(level, BlockPos.ZERO), "emissive", s.emissiveRendering(),
                "dark", s.getShadeBrightness(level, BlockPos.ZERO) < 1.0F);
            var faces = LightShapes.of(s);
            if (faces != null) facts.put("faces", faces);
            out.put(Out.stateKey(s), facts);
        }
        Out.json(Path.of(args[0], "facts.json"), out);
    }
}
