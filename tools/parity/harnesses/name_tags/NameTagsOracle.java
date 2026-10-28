package acacia.parity.name_tags;

import acacia.parity.Client;
import acacia.parity.Out;
import java.nio.file.Path;
import java.util.TreeMap;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.entity.Pose;
import net.minecraft.world.entity.EntityAttachment;
import net.minecraft.world.entity.ai.attributes.Attributes;

/**
 * Where name tags hang (docs/parity.md, harness name_tags), args[0]/name_tags.jsonl, one record
 * per `<type>|<adult|baby>|<pose>` (every entity type adult and, for mobs, baby, standing; every
 * pose for living types): the NAME_TAG attachment point at yaw 0 and 90 (`at`, `turned`), the
 * entity's dimensions (`width`, `height`) and the NAME_TAG_DISTANCE attribute (`distance`, 64
 * for an entity without attributes, as EntityRenderer.extractNameTags' default).
 */
public final class NameTagsOracle {
    public static void main(String[] args) throws Exception {
        Client.minecraft();
        var level = Client.overworld();
        // Client.level's connection skips its field initialisers; the wither's name asks the scoreboard.
        acacia.parity.Headless.set(acacia.parity.Headless.get(level, "connection"), "scoreboard", new net.minecraft.world.scores.Scoreboard());
        try (var out = new Out.Lines(Path.of(args[0]).resolve("name_tags.jsonl"))) {
            for (var type : BuiltInRegistries.ENTITY_TYPE) {
                var id = BuiltInRegistries.ENTITY_TYPE.getKey(type).toString();
                for (boolean baby : new boolean[] {false, true}) {
                    for (var pose : Pose.values()) {
                        var record = new TreeMap<String, Object>();
                        record.put("case", id + "|" + (baby ? "baby" : "adult") + "|" + pose.name().toLowerCase());
                        try {
                            var entity = type.create(level, EntitySpawnReason.LOAD);
                            if (entity == null) break;
                            boolean living = entity instanceof LivingEntity;
                            if (baby && !(entity instanceof Mob)) break;
                            if (pose != Pose.STANDING && !living) break;
                            if (entity instanceof Mob mob) mob.setBaby(baby);
                            if (baby && !(entity instanceof LivingEntity l && l.isBaby())) break;
                            entity.setPose(pose);
                            entity.refreshDimensions();
                            var dimensions = entity.getDimensions(pose);
                            var at = entity.getAttachments().getNullable(EntityAttachment.NAME_TAG, 0, 0.0F);
                            var turned = entity.getAttachments().getNullable(EntityAttachment.NAME_TAG, 0, 90.0F);
                            record.put("width", dimensions.width());
                            record.put("height", dimensions.height());
                            record.put("at", at == null ? null : new double[] {at.x, at.y, at.z});
                            record.put("turned", turned == null ? null : new double[] {turned.x, turned.y, turned.z});
                            double distance = 64.0;
                            if (entity instanceof LivingEntity l && l.getAttribute(Attributes.NAME_TAG_DISTANCE) != null)
                                distance = l.getAttribute(Attributes.NAME_TAG_DISTANCE).getValue();
                            record.put("distance", distance);
                        } catch (Throwable error) {
                            record.put("error", String.valueOf(error));
                        }
                        out.write(record);
                    }
                }
            }
        }
    }
}
