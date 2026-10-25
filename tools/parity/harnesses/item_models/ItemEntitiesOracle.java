package acacia.parity.item_models;

import acacia.parity.Client;
import acacia.parity.Headless;
import acacia.parity.Out;
import com.mojang.blaze3d.vertex.PoseStack;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.file.Path;
import java.util.*;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.ItemEntityRenderer;
import net.minecraft.client.renderer.entity.ItemFrameRenderer;
import net.minecraft.client.renderer.entity.state.ItemClusterRenderState;
import net.minecraft.client.renderer.entity.state.ItemEntityRenderState;
import net.minecraft.client.renderer.entity.state.ItemFrameRenderState;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.util.RandomSource;
import net.minecraft.world.entity.decoration.ItemFrame;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemDisplayContext;
import org.joml.Matrix4f;

/**
 * Dropped and framed items (docs/parity.md, harness item_models), args[0]/item_entities.jsonl.
 * `drop|<item>|<count>`: an ItemEntityRenderState with the item's default stack resolved for
 * GROUND, that count's rendered amount and seed, a random age and bob offset, through
 * ItemEntityRenderer.submit; `frame|<item>|<direction>|<rotation>|<invisible>`: an
 * ItemFrameRenderState resolved for FIXED through ItemFrameRenderer.submit, under the pose the
 * dispatcher gives (the frame entity's position less the block's centre, plus the render offset).
 * Each records the model's bounding box (ItemStackRenderState.getModelBoundingBox) and, per
 * submitItem call, the pose less the layer's own transform (`frames`, column-major): where the
 * renderer put the item before its display. Items with a special renderer record only the box.
 */
public final class ItemEntitiesOracle {
    public static void main(String[] args) throws Exception {
        Client.minecraft();
        var level = Client.overworld();
        var baked = ItemModelsOracle.bake();
        var drops = Headless.allocate(ItemEntityRenderer.class);
        Headless.set(drops, "random", RandomSource.create());
        var frames = Headless.allocate(ItemFrameRenderer.class);
        var random = new Random(26_3);
        var centre = BlockPos.ZERO.offset(0, 70, 0);
        try (var out = new Out.Lines(Path.of(args[0]).resolve("item_entities.jsonl"))) {
            for (var id : new TreeSet<>(BuiltInRegistries.ITEM.keySet())) {
                var model = baked.itemStackModels().get(id);
                if (model == null) continue;
                var stack = ItemModelsOracle.stack(id);
                for (int count : new int[] {1, 2, 17, 33, 64}) {
                    var record = Out.map("case", "drop|" + id + "|" + count);
                    try {
                        var state = new ItemEntityRenderState();
                        resolve(model, state.item, stack, ItemDisplayContext.GROUND);
                        state.count = ItemClusterRenderState.getRenderedAmount(count);
                        state.seed = Item.getId(stack.getItem()) + stack.getDamageValue();
                        state.ageInTicks = random.nextFloat() * 6000.0F;
                        state.bobOffset = random.nextFloat() * (float) (Math.PI * 2);
                        record.putAll(Out.map("count", state.count, "seed", state.seed, "age", state.ageInTicks, "bob_offset", state.bobOffset));
                        var placed = new Placed(state.item);
                        drops.submit(state, new PoseStack(), placed.collector(), null);
                        placed.write(record);
                    } catch (Throwable error) {
                        record.put("error", String.valueOf(error.getCause() != null ? error.getCause() : error));
                    }
                    out.write(record);
                }
                var hangings = new ArrayList<Object[]>();
                hangings.add(new Object[] {Direction.values()[random.nextInt(6)], random.nextInt(8), false});
                if (id.getPath().equals("stick") || id.getPath().equals("stone") || id.getPath().equals("shield")) {
                    for (var direction : Direction.values())
                        for (int rotation = 0; rotation < 8; rotation++) hangings.add(new Object[] {direction, rotation, rotation == 3});
                }
                for (var hanging : hangings) {
                    var direction = (Direction) hanging[0];
                    int rotation = (int) hanging[1];
                    boolean invisible = (boolean) hanging[2];
                    var record = Out.map("case", "frame|" + id + "|" + direction.getSerializedName() + "|" + rotation + "|" + invisible,
                        "direction", direction.getSerializedName(), "rotation", rotation, "invisible", invisible);
                    try {
                        var entity = new ItemFrame(level, centre, direction);
                        var state = new ItemFrameRenderState();
                        resolve(model, state.item, stack, ItemDisplayContext.FIXED);
                        state.direction = direction;
                        state.rotation = rotation;
                        state.isInvisible = invisible;
                        var offset = frames.getRenderOffset(state);
                        var pose = new PoseStack();
                        pose.translate(entity.getX() - (centre.getX() + 0.5) + offset.x, entity.getY() - (centre.getY() + 0.5) + offset.y,
                            entity.getZ() - (centre.getZ() + 0.5) + offset.z);
                        var placed = new Placed(state.item);
                        frames.submit(state, pose, placed.collector(), null);
                        placed.write(record);
                    } catch (Throwable error) {
                        record.put("error", String.valueOf(error.getCause() != null ? error.getCause() : error));
                    }
                    out.write(record);
                }
            }
        }
    }

    private static void resolve(net.minecraft.client.renderer.item.ItemModel model, ItemStackRenderState state, net.minecraft.world.item.ItemStack stack,
            ItemDisplayContext context) throws Exception {
        state.clear();
        ItemModelsOracle.field(ItemStackRenderState.class, "displayContext").set(state, context);
        model.update(state, stack, null, context, null, null, 0);
    }

    /** The poses submitItem gets, less each layer's transform in turn. */
    private static final class Placed {
        final ItemStackRenderState item;
        final List<float[]> frames = new ArrayList<>();
        final List<Matrix4f> layers = new ArrayList<>();
        boolean special;

        Placed(ItemStackRenderState item) throws Exception {
            this.item = item;
            int count = (int) ItemModelsOracle.field(ItemStackRenderState.class, "activeLayerCount").get(item);
            var all = (Object[]) ItemModelsOracle.field(ItemStackRenderState.class, "layers").get(item);
            for (int i = 0; i < count; i++) {
                var layer = all[i];
                special |= ItemModelsOracle.field(layer.getClass(), "specialRenderer").get(layer) != null;
                Method apply = layer.getClass().getDeclaredMethod("applyTransform", PoseStack.Pose.class);
                apply.setAccessible(true);
                var pose = new PoseStack().last();
                apply.invoke(layer, pose);
                layers.add(new Matrix4f(pose.pose()).invert());
            }
        }

        SubmitNodeCollector collector() {
            InvocationHandler handler = (proxy, method, args) -> {
                var name = method.getName();
                if (name.equals("order")) return proxy;
                if (name.equals("submitItem") && args.length == 8) {
                    var layer = layers.get(frames.size() % layers.size());
                    frames.add(new Matrix4f(((PoseStack) args[0]).last().pose()).mul(layer).get(new float[16]));
                    return null;
                }
                if (method.isDefault()) return InvocationHandler.invokeDefault(proxy, method, args);
                return null;
            };
            return (SubmitNodeCollector) Proxy.newProxyInstance(SubmitNodeCollector.class.getClassLoader(), new Class<?>[] {SubmitNodeCollector.class}, handler);
        }

        void write(Map<String, Object> record) {
            var box = item.getModelBoundingBox();
            record.put("box", new double[] {box.minX, box.minY, box.minZ, box.maxX, box.maxY, box.maxZ});
            record.put("layers", layers.size());
            record.put("special", special);
            if (!special) record.put("frames", frames);
        }
    }
}
