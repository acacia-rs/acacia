// SPDX-License-Identifier: MIT
package acacia.lookbake;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import net.minecraft.SharedConstants;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.Property;
import net.minecraft.world.phys.shapes.Shapes;
import net.minecraft.world.phys.shapes.VoxelShape;

/**
 * Writes, for every block state that can occlude (`canOcclude`), what `Block.shouldRenderFace`
 * reads of it as a neighbour: its six face occlusion shapes as 16x16 grids and whether each is
 * the whole block (`Shapes.block()`). Keyed by state as `name[k=v,...]`, properties sorted. Run
 * by `lookbake java` against the user's own client jar; the output stays on that machine
 * (docs/java-look.md "Licensing").
 *
 * usage: java -cp <client jar>:<its libraries>:<classes> acacia.lookbake.BlockShapeExporter OUT.json
 */
public final class BlockShapeExporter {
    /** Cells of the face plane whose centre a box of `shape` covers, as acacia's FaceShapes: 64 hex digits, bit `v * 16 + u` a digit's high bit first; (u, v) is (x, z) on y faces, (x, y) on z faces, (z, y) on x faces. */
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

    static String key(BlockState state) {
        var name = BuiltInRegistries.BLOCK.getKey(state.getBlock()).toString();
        var pairs = new ArrayList<String>();
        for (Property<?> property : state.getProperties()) pairs.add(property.getName() + "=" + value(state, property));
        if (pairs.isEmpty()) return name;
        pairs.sort(null);
        return name + "[" + String.join(",", pairs) + "]";
    }

    private static <T extends Comparable<T>> String value(BlockState state, Property<T> property) {
        return property.getName(state.getValue(property));
    }

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var out = new JsonObject();
        for (BlockState s : Block.BLOCK_STATE_REGISTRY) {
            if (!s.canOcclude()) continue;
            var faces = new JsonArray();
            var whole = new JsonArray();
            for (var d : Direction.values()) {
                faces.add(grid(s.getFaceOcclusionShape(d), d));
                whole.add(s.getFaceOcclusionShape(d) == Shapes.block());
            }
            var entry = new JsonObject();
            entry.add("faces", faces);
            entry.add("whole", whole);
            out.add(key(s), entry);
        }
        Files.writeString(Path.of(args[0]), new Gson().toJson(out), StandardCharsets.UTF_8);
        System.out.println(out.size() + " occluding states");
    }
}
