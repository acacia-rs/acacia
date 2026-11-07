// Dumps Java 26.x blocks for gen.mjs: per block its properties (as the static field that declares each,
// so same-named properties with different values stay apart), default state and first state id; per state
// its collision boxes with an empty context at the origin. Run: see ../../README.md "Data sources".
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.IdentityHashMap;
import java.util.Map;

import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.util.StringRepresentable;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.Property;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.shapes.CollisionContext;

public class BlockDump {
    static final Map<Property<?>, String> CONSTANTS = new IdentityHashMap<>();

    static void collect(Class<?> c) throws Exception {
        for (Field f : c.getDeclaredFields()) {
            if (!Modifier.isStatic(f.getModifiers()) || !Property.class.isAssignableFrom(f.getType())) continue;
            f.setAccessible(true);
            Property<?> p = (Property<?>) f.get(null);
            String owner = c.getSimpleName().equals("BlockStateProperties") ? "" : c.getSimpleName() + ".";
            CONSTANTS.putIfAbsent(p, owner + f.getName());
        }
    }

    static String json(String s) {
        return "\"" + s.replace("\\", "\\\\").replace("\"", "\\\"") + "\"";
    }

    static String value(Object v) {
        return v instanceof StringRepresentable r ? r.getSerializedName() : String.valueOf(v);
    }

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        collect(Class.forName("net.minecraft.world.level.block.state.properties.BlockStateProperties"));
        for (Block b : BuiltInRegistries.BLOCK) {
            for (Class<?> c = b.getClass(); c != Object.class; c = c.getSuperclass()) collect(c);
        }
        StringBuilder out = new StringBuilder("{\"blocks\":[");
        boolean firstBlock = true;
        for (Block b : BuiltInRegistries.BLOCK) {
            if (!firstBlock) out.append(",");
            firstBlock = false;
            BlockState first = b.getStateDefinition().getPossibleStates().get(0);
            out.append("{\"name\":").append(json(BuiltInRegistries.BLOCK.getKey(b).toString()))
                .append(",\"first\":").append(Block.getId(first))
                .append(",\"default\":").append(Block.getId(b.defaultBlockState()))
                .append(",\"props\":[");
            boolean firstProp = true;
            for (Property<?> p : b.getStateDefinition().getProperties()) {
                if (!firstProp) out.append(",");
                firstProp = false;
                String constant = CONSTANTS.get(p);
                if (constant == null) throw new IllegalStateException("no constant for " + p + " on " + b);
                out.append("{\"const\":").append(json(constant)).append(",\"name\":").append(json(p.getName()))
                    .append(",\"type\":").append(json(p.getValueClass().getSimpleName()))
                    .append(",\"values\":[");
                boolean firstValue = true;
                for (Object v : p.getPossibleValues()) {
                    if (!firstValue) out.append(",");
                    firstValue = false;
                    out.append(json(value(v)));
                }
                out.append("]}");
            }
            out.append("]}");
        }
        out.append("],\"collision\":[");
        for (int id = 0; id < Block.BLOCK_STATE_REGISTRY.size(); id++) {
            BlockState s = Block.BLOCK_STATE_REGISTRY.byId(id);
            if (id > 0) out.append(",");
            out.append("[");
            boolean firstBox = true;
            for (AABB a : s.getCollisionShape(EmptyBlockGetter.INSTANCE, BlockPos.ZERO, CollisionContext.empty()).toAabbs()) {
                if (!firstBox) out.append(",");
                firstBox = false;
                out.append("[").append(a.minX).append(",").append(a.minY).append(",").append(a.minZ).append(",")
                    .append(a.maxX).append(",").append(a.maxY).append(",").append(a.maxZ).append("]");
            }
            out.append("]");
        }
        Files.writeString(Path.of(args[0]), out.append("]}\n"));
    }
}
