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
                .append(",\"collision\":\"").append(collision(c)).append("\"}");
        }
        Files.writeString(Path.of(args[0]), out.append("]").toString());
    }
}
