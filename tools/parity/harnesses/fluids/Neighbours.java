package acacia.parity.fluids;

import static acacia.parity.fluids.Cases.C;
import static acacia.parity.fluids.Cases.Y;
import static acacia.parity.fluids.Cases.fluid;
import static acacia.parity.fluids.Cases.id;
import static acacia.parity.fluids.Cases.s;
import static acacia.parity.fluids.Cases.scene;

import java.util.List;

/** Liquids against each of {@link Cases#NEIGHBOURS}, and each of {@link Cases#WATERLOGGED} among liquids. */
final class Neighbours {
    private Neighbours() {}

    static void add(List<Scene> out) {
        for (var text : Cases.NEIGHBOURS) {
            var n = s(text);
            var name = id(text);
            for (int level : new int[]{0, 4, 8}) out.add(scene("next_" + name + "_" + level).put(C, Y, C, fluid("water", level)).put(C + 1, Y, C, n));
            out.add(scene("next_" + name + "_lava").put(C, Y, C, fluid("lava", 0)).put(C, Y, C - 1, n));
            out.add(scene("under_" + name).put(C, Y, C, fluid("water", 0)).put(C, Y + 1, C, n));
            out.add(scene("over_" + name).put(C, Y + 1, C, fluid("water", 2)).put(C, Y, C, n));
            // getFlow looks under a neighbour without liquid unless it is in #blocks_fluid_flow.
            out.add(scene("flow_" + name).put(C, Y, C, fluid("water", 3)).put(C - 1, Y, C, fluid("water", 2))
                .put(C + 1, Y, C, n).put(C + 1, Y - 1, C, fluid("water", 0)));
        }
        for (var text : Cases.WATERLOGGED) {
            var w = s(text);
            var name = id(text);
            out.add(scene("logged_" + name).put(C, Y, C, w));
            out.add(Cases.pool(scene("logged_pool_" + name), "water", 1, 1).put(C, Y, C, w));
            out.add(scene("logged_next_" + name).put(C, Y, C, w).put(C + 1, Y, C, fluid("water", 2)).put(C, Y, C - 1, fluid("water", 5)));
            out.add(scene("logged_under_" + name).put(C, Y, C, w).put(C, Y + 1, C, fluid("water", 3)));
        }
    }
}
