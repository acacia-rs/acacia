package acacia.parity.gui;

import com.google.gson.JsonObject;
import java.util.HashMap;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;

/**
 * An entity render dispatcher whose every renderer only fills the render state fields the GUI's
 * pictures read (type, bounding box, scale, pose): enough for
 * InventoryScreen.extractEntityInInventoryFollowsMouse without models or textures. Players go
 * through the dispatcher's avatar path, which this does not cover.
 */
public final class EntityStub {
    private EntityStub() {}

    public static final class Renderer extends EntityRenderer<Entity, LivingEntityRenderState> {
        private Renderer() {
            super(null);
        }

        @Override public LivingEntityRenderState createRenderState() { return new LivingEntityRenderState(); }

        @Override public void extractRenderState(Entity entity, LivingEntityRenderState state, float partial) {
            state.entityType = entity.getType();
            state.boundingBoxWidth = entity.getBbWidth();
            state.boundingBoxHeight = entity.getBbHeight();
            if (entity instanceof LivingEntity living) {
                state.scale = living.getScale();
                state.pose = living.getPose();
            }
        }

        @Override protected void finalizeRenderState(Entity entity, LivingEntityRenderState state) { }
    }

    /** Installs the stand-in dispatcher on `mc`. */
    @SuppressWarnings({"rawtypes", "unchecked"})
    public static void install(Minecraft mc) throws Exception {
        var renderer = Stub.alloc(Renderer.class);
        var dispatcher = Stub.alloc(EntityRenderDispatcher.class);
        Stub.put(dispatcher, EntityRenderDispatcher.class, "renderers", new HashMap() {
            @Override public Object get(Object key) { return renderer; }
        });
        Stub.put(mc, Minecraft.class, "entityRenderDispatcher", dispatcher);
    }

    /** What the probe needs of `entity`'s render state: {entity, bbHeight, scale?, fallFlying}. */
    public static JsonObject figure(Entity entity) {
        var o = new JsonObject();
        o.addProperty("entity", net.minecraft.core.registries.BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString());
        o.addProperty("bbHeight", entity.getBbHeight());
        if (entity instanceof LivingEntity living) {
            o.addProperty("scale", living.getScale());
            o.addProperty("fallFlying", living.getPose() == net.minecraft.world.entity.Pose.FALL_FLYING);
        }
        return o;
    }
}
