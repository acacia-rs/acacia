package acacia.parity.entity_poses;

import acacia.parity.EntityModels;
import acacia.parity.Game;
import acacia.parity.Out;
import acacia.parity.RenderStates;
import com.google.gson.JsonParser;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.*;

/**
 * Model.setupAnim on render states (docs/parity.md, harness entity_poses). args[1] is a JSON-lines
 * file of cases (RenderStates.pose); each case's record in args[0]/poses.jsonl repeats it and adds
 * the posed parts and drawn corners, as EntityModelsOracle's.
 */
public final class EntityPosesOracle {
    public static void main(String[] args) throws Exception {
        Game.init();
        try (var out = new Out.Lines(Path.of(args[0]).resolve("poses.jsonl"))) {
            for (String line : Files.readAllLines(Path.of(args[1]))) {
                if (line.isBlank()) continue;
                var kase = JsonParser.parseString(line).getAsJsonObject();
                var record = new TreeMap<String, Object>();
                for (var e : kase.entrySet()) record.put(e.getKey(), e.getValue());
                try {
                    var posed = RenderStates.pose(kase);
                    var parts = new TreeMap<String, Object>();
                    EntityModels.poses("", posed.root(), parts);
                    var drawn = new TreeMap<String, List<float[]>>();
                    EntityModels.draw("", posed.root(), EntityModels.livingRoot(0.0F, new float[] {1, 1, 1}), drawn);
                    record.put("parts", parts);
                    record.put("drawn", drawn);
                } catch (Throwable error) {
                    var cause = error.getCause() != null ? error.getCause() : error;
                    record.put("error", cause.toString());
                }
                out.write(record);
            }
        }
    }
}
