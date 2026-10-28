// Dumps, per Java block state id, what Java's light engine and heightmaps read from it, for gen-mapping.mjs:
// [light dampening, emission, full occlusion faces (bit per Direction ordinal, only for shape-occluding
// states), 1 = air | 2 = holds a fluid].
// Build and run against the unobfuscated 26.x server jar: see README.md "Data sources".
import java.io.PrintWriter;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;

import net.minecraft.SharedConstants;
import net.minecraft.core.Direction;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.lighting.LightEngine;

public class LightDump {
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        Method isEmptyShape = LightEngine.class.getDeclaredMethod("isEmptyShape", BlockState.class);
        isEmptyShape.setAccessible(true);
        StringBuilder out = new StringBuilder("[");
        for (int id = 0; id < Block.BLOCK_STATE_REGISTRY.size(); id++) {
            BlockState s = Block.BLOCK_STATE_REGISTRY.byId(id);
            int faces = 0;
            if (!(boolean) isEmptyShape.invoke(null, s)) {
                for (Direction d : Direction.values()) {
                    if (Block.isFaceFull(LightEngine.getOcclusionShape(s, d), d)) faces |= 1 << d.ordinal();
                }
            }
            // heightmaps test block tags, which need a loaded datapack: the generator resolves those
            int heights = (s.isAir() ? 1 : 0) | (s.getFluidState().isEmpty() ? 0 : 2);
            if (id > 0) out.append(",");
            out.append("[").append(s.getLightDampening()).append(",").append(s.getLightEmission()).append(",")
                .append(faces).append(",").append(heights).append("]");
        }
        Files.writeString(Path.of(args[0]), out.append("]\n"));
    }
}
