// SPDX-License-Identifier: MIT
// Adapted from Horizon's VanillaEntityModelExporter (MIT, (c) Ludovic Marie; see THIRD-PARTY.md).
package acacia.lookbake;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import net.minecraft.SharedConstants;
import net.minecraft.client.model.geom.LayerDefinitions;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.server.Bootstrap;

/**
 * Writes every model layer the client builds in code (LayerDefinitions.createRoots, each baked
 * with bakeRoot) as a part tree: per part its initial pose (x, y, z, xRot, yRot, zRot, xScale,
 * yScale, zScale), its cubes' quads (corners as x, y, z, u, v) with their normals, and its
 * children by name. Run by `lookbake java` against the user's own client jar; the output stays
 * on that machine (docs/java-look.md "Licensing").
 *
 * usage: java -cp <client jar>:<its libraries>:<classes> acacia.lookbake.EntityModelExporter OUT.json
 */
public final class EntityModelExporter {
    private static Field cubes;
    private static Field children;

    @SuppressWarnings("unchecked")
    private static JsonObject part(ModelPart part) throws ReflectiveOperationException {
        var out = new JsonObject();
        var pose = new JsonArray();
        for (float f : new float[] {part.x, part.y, part.z, part.xRot, part.yRot, part.zRot, part.xScale, part.yScale, part.zScale})
            pose.add(f);
        out.add("pose", pose);
        var polygons = new JsonArray();
        var normals = new JsonArray();
        for (ModelPart.Cube cube : (List<ModelPart.Cube>) cubes.get(part)) {
            for (ModelPart.Polygon polygon : cube.polygons) {
                var corners = new JsonArray();
                for (ModelPart.Vertex v : polygon.vertices()) {
                    var corner = new JsonArray();
                    // A zero-size face has NaN texture coordinates.
                    for (float f : new float[] {v.x(), v.y(), v.z(), v.u(), v.v()})
                        corner.add(Float.isFinite(f) ? f : 0.0F);
                    corners.add(corner);
                }
                polygons.add(corners);
                var normal = new JsonArray();
                normal.add(polygon.normal().x());
                normal.add(polygon.normal().y());
                normal.add(polygon.normal().z());
                normals.add(normal);
            }
        }
        out.add("polygons", polygons);
        out.add("normals", normals);
        var kids = new JsonObject();
        for (var entry : ((Map<String, ModelPart>) children.get(part)).entrySet())
            kids.add(entry.getKey(), part(entry.getValue()));
        out.add("children", kids);
        return out;
    }

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        cubes = ModelPart.class.getDeclaredField("cubes");
        cubes.setAccessible(true);
        children = ModelPart.class.getDeclaredField("children");
        children.setAccessible(true);
        var out = new JsonObject();
        int skipped = 0;
        for (var entry : LayerDefinitions.createRoots().entrySet()) {
            try {
                out.add(entry.getKey().toString(), part(entry.getValue().bakeRoot()));
            } catch (RuntimeException error) {
                System.err.println("skipped " + entry.getKey() + ": " + error);
                skipped++;
            }
        }
        Files.writeString(Path.of(args[0]), new Gson().toJson(out), StandardCharsets.UTF_8);
        System.out.println(out.size() + " layers, " + skipped + " skipped");
    }
}
