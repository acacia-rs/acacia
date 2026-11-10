// Dumps each 26.3 entity type's default size and how vehicles collide with it, by registry id, for
// gen_entity_types.mjs. Run: java -cp <26.3 server-game.jar + its libraries> EntityDump.java <out.json>
import java.lang.reflect.Field;
import java.lang.reflect.ParameterizedType;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.IdentityHashMap;
import java.util.Map;

import net.minecraft.SharedConstants;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.network.syncher.EntityDataAccessor;
import net.minecraft.world.entity.EntityDimensions;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.EntityTypes;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.ambient.Bat;
import net.minecraft.world.entity.decoration.ArmorStand;
import net.minecraft.world.entity.monster.Shulker;
import net.minecraft.world.entity.vehicle.boat.AbstractBoat;
import net.minecraft.world.entity.vehicle.minecart.AbstractMinecart;

public class EntityDump {
    // `canBeCollidedWith` → solid; `isPushable` (as a client sees a live mob off ladders) → pushable.
    static String collision(Class<?> c) {
        if (AbstractBoat.class.isAssignableFrom(c) || Shulker.class.isAssignableFrom(c)) return "solid";
        if (AbstractMinecart.class.isAssignableFrom(c)) return "pushable";
        if (Bat.class.isAssignableFrom(c) || ArmorStand.class.isAssignableFrom(c)) return "none";
        return LivingEntity.class.isAssignableFrom(c) ? "pushable" : "none";
    }

    // `getAgeScale` overrides in 26.3; every other baby is half size.
    static final Map<String, Float> AGE_SCALES = Map.of(
        "Dolphin", 0.65F, "HappyGhast", 0.2375F, "Goat", 0.55F, "Armadillo", 0.6F, "Turtle", 0.3F, "Camel", 0.6F);

    static Object staticField(Class<?> c, String name) throws Exception {
        for (; c != Object.class; c = c.getSuperclass()) {
            try {
                Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f.get(null);
            } catch (NoSuchFieldException e) {
                // keep looking in the superclass
            }
        }
        return null;
    }

    // `,"baby":[w,h],"babyData":id` for types with a synced baby flag.
    static String baby(Class<?> c, EntityType<?> t) throws Exception {
        if (!(staticField(c, "DATA_BABY_ID") instanceof EntityDataAccessor<?> flag)) return "";
        float w, h;
        if (staticField(c, "BABY_DIMENSIONS") instanceof EntityDimensions d) {
            w = d.width();
            h = d.height();
        } else {
            float scale = AGE_SCALES.getOrDefault(c.getSimpleName(), 0.5F);
            w = t.getWidth() * scale;
            h = t.getHeight() * scale;
        }
        return ",\"baby\":[" + w + "," + h + "],\"babyData\":" + flag.id();
    }

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        // The factory hides each type's class; the constant's `EntityType<X>` declaration names it.
        Map<EntityType<?>, Class<?>> classes = new IdentityHashMap<>();
        for (Field f : EntityTypes.class.getDeclaredFields()) {
            if (f.getType() == EntityType.class && f.getGenericType() instanceof ParameterizedType p
                && p.getActualTypeArguments()[0] instanceof Class<?> c) {
                classes.put((EntityType<?>) f.get(null), c);
            }
        }
        StringBuilder out = new StringBuilder("[");
        for (EntityType<?> t : BuiltInRegistries.ENTITY_TYPE) {
            Class<?> c = classes.get(t);
            if (c == null) throw new IllegalStateException("no constant for " + BuiltInRegistries.ENTITY_TYPE.getKey(t));
            if (out.length() > 1) out.append(",");
            out.append("{\"id\":").append(BuiltInRegistries.ENTITY_TYPE.getId(t))
                .append(",\"name\":\"").append(BuiltInRegistries.ENTITY_TYPE.getKey(t)).append("\"")
                .append(",\"width\":").append(t.getWidth())
                .append(",\"height\":").append(t.getHeight())
                .append(",\"collision\":\"").append(collision(c)).append("\"")
                .append(baby(c, t)).append("}");
        }
        Files.writeString(Path.of(args[0]), out.append("]").toString());
    }
}
