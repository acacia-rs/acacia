// Dumps what the sky light engine reads from each 26.3 block state, for sky_props.rs: light dampening, emission,
// useShapeForLightOcclusion, and classes of the DOWN and UP occlusion face shapes with a table saying which pairs
// occlude (`Shapes.faceShapeOccludes`). Run: java -cp <26.3 client jar + its libraries> SkyLightDump.java <out.bin>
// Layout: u8 class count n, n*n bytes [top DOWN class][bottom UP class], u8 row count r, r rows of
// (u8 dampening | use_shape << 4, u8 emission, u8 down class, u8 up class), u32 BE state count, a row index per state.
import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import net.minecraft.SharedConstants;
import net.minecraft.core.Direction;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.lighting.LightEngine;
import net.minecraft.world.phys.shapes.Shapes;
import net.minecraft.world.phys.shapes.VoxelShape;

public class SkyLightDump {
    static final Map<String, Integer> classes = new LinkedHashMap<>();
    static final List<VoxelShape> shapes = new ArrayList<>();

    static int classOf(VoxelShape shape) {
        String key = shape == Shapes.block() ? "block" : shape.toAabbs().toString();
        return classes.computeIfAbsent(key, k -> {
            shapes.add(shape);
            return shapes.size() - 1;
        });
    }

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        int count = Block.BLOCK_STATE_REGISTRY.size();
        int[][] rows = new int[count][];
        for (int id = 0; id < count; id++) {
            BlockState s = Block.BLOCK_STATE_REGISTRY.byId(id);
            int down = classOf(LightEngine.getOcclusionShape(s, Direction.DOWN));
            int up = classOf(LightEngine.getOcclusionShape(s, Direction.UP));
            rows[id] = new int[] {s.getLightDampening() | (s.useShapeForLightOcclusion() ? 16 : 0), s.getLightEmission(), down, up};
        }
        int n = shapes.size();
        if (n > 255) throw new IllegalStateException("too many face shapes: " + n);
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        DataOutputStream out = new DataOutputStream(bytes);
        out.writeByte(n);
        for (int top = 0; top < n; top++) {
            for (int bottom = 0; bottom < n; bottom++) {
                out.writeByte(Shapes.faceShapeOccludes(shapes.get(top), shapes.get(bottom)) ? 1 : 0);
            }
        }
        Map<String, Integer> distinct = new LinkedHashMap<>();
        List<int[]> table = new ArrayList<>();
        byte[] rowOf = new byte[count];
        for (int id = 0; id < count; id++) {
            int[] row = rows[id];
            int index = distinct.computeIfAbsent(Arrays.toString(row), k -> {
                table.add(row);
                return table.size() - 1;
            });
            rowOf[id] = (byte) index;
        }
        if (table.size() > 255) throw new IllegalStateException("too many distinct rows: " + table.size());
        out.writeByte(table.size());
        for (int[] row : table) {
            for (int v : row) out.writeByte(v);
        }
        out.writeInt(count);
        out.write(rowOf);
        Files.write(Path.of(args[0]), bytes.toByteArray());
        System.out.println(count + " states, " + n + " face shapes, " + bytes.size() + " bytes");
    }
}
