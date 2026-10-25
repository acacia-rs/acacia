package acacia.parity.light_facts;

import acacia.parity.Game;
import acacia.parity.Out;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.TreeMap;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.shapes.VoxelShape;

/**
 * Every block state's light facts into args[0]/facts.json: what the light engine and
 * BlockModelLighter read of it (emission, dampening, solid render, full collision, emissive
 * rendering, shade brightness), and, for a state whose shape the engine looks at
 * (`!LightEngine.isEmptyShape`), its face occlusion shape per direction as a 16x16 grid of the
 * face, row-major, as 64 hex digits.
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
            if (s.canOcclude() && s.useShapeForLightOcclusion()) {
                var faces = new ArrayList<String>();
                for (var d : Direction.values()) faces.add(grid(s.getFaceOcclusionShape(d), d));
                facts.put("faces", faces);
            }
            out.put(Out.stateKey(s), facts);
        }
        Out.json(Path.of(args[0], "facts.json"), out);
    }

    /** Cells of the face plane whose centre a box of `shape` covers: (u, v) is (x, z) on y faces, (x, y) on z faces, (z, y) on x faces. */
    static String grid(VoxelShape shape, Direction d) {
        var bits = new boolean[256];
        shape.forAllBoxes((x0, y0, z0, x1, y1, z1) -> {
            double[] lo, hi;
            switch (d.getAxis()) {
                case Y -> { lo = new double[]{x0, z0}; hi = new double[]{x1, z1}; }
                case Z -> { lo = new double[]{x0, y0}; hi = new double[]{x1, y1}; }
                default -> { lo = new double[]{z0, y0}; hi = new double[]{z1, y1}; }
            }
            for (int v = 0; v < 16; v++) {
                for (int u = 0; u < 16; u++) {
                    double cu = (u + 0.5) / 16, cv = (v + 0.5) / 16;
                    if (cu > lo[0] && cu < hi[0] && cv > lo[1] && cv < hi[1]) bits[v * 16 + u] = true;
                }
            }
        });
        var hex = new StringBuilder();
        for (int i = 0; i < 256; i += 4) {
            int nibble = (bits[i] ? 8 : 0) | (bits[i + 1] ? 4 : 0) | (bits[i + 2] ? 2 : 0) | (bits[i + 3] ? 1 : 0);
            hex.append(Character.forDigit(nibble, 16));
        }
        return hex.toString();
    }
}
