package acacia.parity;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.Direction;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.shapes.VoxelShape;

/** A state's face occlusion shapes as the light engine sees them, in acacia's FaceShapes layout. */
public final class LightShapes {
    private LightShapes() {}

    /** Null where the engine ignores the shape (`LightEngine.isEmptyShape`), else the six faces by `grid`. */
    public static List<String> of(BlockState s) {
        if (!s.canOcclude() || !s.useShapeForLightOcclusion()) return null;
        return faces(s);
    }

    /** The six `getFaceOcclusionShape`s by `grid`, `DOWN`..`EAST`. */
    public static List<String> faces(BlockState s) {
        var faces = new ArrayList<String>();
        for (var d : Direction.values()) faces.add(grid(s.getFaceOcclusionShape(d), d));
        return faces;
    }

    /** Cells of the face plane whose centre a box of `shape` covers, 16x16 row-major as 64 hex digits (bit `v * 16 + u`, first bit as a digit's high bit): (u, v) is (x, z) on y faces, (x, y) on z faces, (z, y) on x faces. */
    public static String grid(VoxelShape shape, Direction d) {
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
