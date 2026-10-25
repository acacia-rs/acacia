package acacia.parity.entity_models;

import acacia.parity.EntityModels;
import acacia.parity.Game;
import acacia.parity.Out;
import java.nio.file.Path;
import java.util.*;

/**
 * Every entity model layer the client bakes, drawn at rest by ModelPart.render under
 * LivingEntityRenderer's root transform (yaw 0): per layer, each part's initial pose and each
 * drawn part's corners [x, y, z, nx, ny, nz, u, v]. Writes args[0]/layers.jsonl.
 */
public final class EntityModelsOracle {
    public static void main(String[] args) throws Exception {
        Game.init();
        try (var out = new Out.Lines(Path.of(args[0]).resolve("layers.jsonl"))) {
            for (var entry : EntityModels.layers().entrySet()) {
                var root = entry.getValue().getValue().bakeRoot();
                var parts = new TreeMap<String, Object>();
                EntityModels.poses("", root, parts);
                var drawn = new TreeMap<String, List<float[]>>();
                EntityModels.draw("", root, EntityModels.livingRoot(0.0F, new float[] {1, 1, 1}), drawn);
                out.write(Out.map("case", entry.getKey(), "parts", parts, "drawn", drawn));
            }
        }
    }
}
