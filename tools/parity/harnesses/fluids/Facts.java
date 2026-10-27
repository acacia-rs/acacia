package acacia.parity.fluids;

import acacia.parity.Out;
import java.lang.reflect.Method;
import java.nio.file.Path;
import java.util.TreeSet;
import net.minecraft.client.renderer.block.FluidRenderer;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.tags.BlockTags;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.HalfTransparentBlock;
import net.minecraft.world.level.block.IceBlock;
import net.minecraft.world.level.block.LeavesBlock;
import net.minecraft.world.level.block.state.BlockState;

/**
 * What FluidRenderer and FlowingFluid.getFlow read of each block state, as acacia's `FluidRules`
 * holds it (faces in east, west, up, down, south, north order), to `facts.jsonl`.
 */
final class Facts {
    private Facts() {}

    static final Direction[] FACES = {Direction.EAST, Direction.WEST, Direction.UP, Direction.DOWN, Direction.SOUTH, Direction.NORTH};

    private static Method occludedByState;

    /** FluidRenderer.isFaceOccludedByState: a fluid box `height` tall, its `direction` face against `state`. */
    static boolean occluded(Direction direction, float height, BlockState state) throws Exception {
        if (occludedByState == null) {
            occludedByState = FluidRenderer.class.getDeclaredMethod("isFaceOccludedByState", Direction.class, float.class, BlockState.class);
            occludedByState.setAccessible(true);
        }
        return (boolean) occludedByState.invoke(null, direction, height, state);
    }

    /**
     * Side faces: the greatest liquid height `face` still hides (0 for none); up and down: 1 when
     * the face hides a full liquid block, else 0.
     */
    static float cover(BlockState state, Direction face) throws Exception {
        var toward = face.getOpposite();
        if (face.getAxis() == Direction.Axis.Y) return occluded(toward, 1.0f, state) ? 1 : 0;
        var heights = new TreeSet<Float>();
        heights.add(1.0f);
        for (double y : state.getFaceOcclusionShape(face).getCoords(Direction.Axis.Y)) if (y > 0 && y <= 1) heights.add((float) y);
        float cover = 0;
        for (float h : heights) if (occluded(toward, h, state)) cover = h;
        return cover;
    }

    static void write(Path out) throws Exception {
        try (var lines = new Out.Lines(out)) {
            for (var state : Block.BLOCK_STATE_REGISTRY) {
                var cover = new float[6];
                var sturdy = new boolean[6];
                for (int f = 0; f < 6; f++) {
                    cover[f] = cover(state, FACES[f]);
                    sturdy[f] = state.isFaceSturdy(EmptyBlockGetter.INSTANCE, BlockPos.ZERO, FACES[f]);
                }
                var block = state.getBlock();
                lines.write(Out.map("state", Out.stateKey(state), "cover", cover, "sturdy", sturdy, "solid", state.isSolid(),
                    "blocks_motion", state.is(BlockTags.BLOCKS_FLUID_FLOW), "overlay", block instanceof HalfTransparentBlock || block instanceof LeavesBlock,
                    "ice", block instanceof IceBlock, "solid_render", state.isSolidRender()));
            }
        }
    }
}
