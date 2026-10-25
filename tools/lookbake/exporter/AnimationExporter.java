// SPDX-License-Identifier: MIT
package acacia.lookbake;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.jar.JarFile;
import net.minecraft.SharedConstants;
import net.minecraft.client.animation.AnimationChannel;
import net.minecraft.client.animation.AnimationDefinition;
import net.minecraft.client.animation.Keyframe;
import net.minecraft.server.Bootstrap;

/**
 * Writes every keyframe animation the client defines in code (the static AnimationDefinition
 * fields of net.minecraft.client.animation.definitions), keyed `Class.FIELD`: its length in
 * seconds, whether it loops, and per bone its channels (target position, rotation or scale; per
 * keyframe the timestamp, pre and post targets as the definition built them, and the
 * interpolation). Run by `lookbake java` against the user's own client jar; the output stays on
 * that machine (docs/java-look.md "Licensing").
 *
 * usage: java -cp <client jar>:<its libraries>:<classes> acacia.lookbake.AnimationExporter OUT.json
 */
public final class AnimationExporter {
    private static final String PACKAGE = "net/minecraft/client/animation/definitions/";

    private static String target(AnimationChannel.Target target) {
        if (target == AnimationChannel.Targets.POSITION) return "position";
        if (target == AnimationChannel.Targets.ROTATION) return "rotation";
        if (target == AnimationChannel.Targets.SCALE) return "scale";
        throw new IllegalArgumentException("unknown target " + target);
    }

    private static String interpolation(AnimationChannel.Interpolation interpolation) {
        if (interpolation == AnimationChannel.Interpolations.LINEAR) return "linear";
        if (interpolation == AnimationChannel.Interpolations.CATMULLROM) return "catmullrom";
        throw new IllegalArgumentException("unknown interpolation " + interpolation);
    }

    private static JsonObject definition(AnimationDefinition definition) {
        var out = new JsonObject();
        out.addProperty("length", definition.lengthInSeconds());
        out.addProperty("looping", definition.looping());
        var bones = new JsonObject();
        for (var bone : definition.boneAnimations().entrySet()) {
            var channels = new JsonArray();
            for (AnimationChannel channel : bone.getValue()) {
                var json = new JsonObject();
                json.addProperty("target", target(channel.target()));
                var frames = new JsonArray();
                for (Keyframe frame : channel.keyframes()) {
                    var row = new JsonArray();
                    row.add(frame.timestamp());
                    for (var v : new org.joml.Vector3fc[] {frame.preTarget(), frame.postTarget()}) {
                        row.add(v.x());
                        row.add(v.y());
                        row.add(v.z());
                    }
                    row.add(interpolation(frame.interpolation()));
                    frames.add(row);
                }
                json.add("keyframes", frames);
                channels.add(json);
            }
            bones.add(bone.getKey(), channels);
        }
        out.add("bones", bones);
        return out;
    }

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var out = new JsonObject();
        var jar = Path.of(AnimationDefinition.class.getProtectionDomain().getCodeSource().getLocation().toURI());
        try (var file = new JarFile(jar.toFile())) {
            for (var entry : java.util.Collections.list(file.entries())) {
                String name = entry.getName();
                if (!name.startsWith(PACKAGE) || !name.endsWith(".class") || name.contains("$")) continue;
                var type = Class.forName(name.substring(0, name.length() - 6).replace('/', '.'));
                for (var field : type.getDeclaredFields()) {
                    if (!Modifier.isStatic(field.getModifiers()) || field.getType() != AnimationDefinition.class) continue;
                    field.setAccessible(true);
                    out.add(type.getSimpleName() + "." + field.getName(), definition((AnimationDefinition) field.get(null)));
                }
            }
        }
        Files.writeString(Path.of(args[0]), new Gson().toJson(out), StandardCharsets.UTF_8);
        System.out.println(out.size() + " animations");
    }
}
