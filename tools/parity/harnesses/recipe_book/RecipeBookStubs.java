package acacia.parity.gui;

import com.google.gson.JsonObject;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.Pose;

/**
 * What the recipe book screens need of a client beyond ScreenKit: a text input manager the focused
 * search box can move, and an entity render dispatcher that hands InventoryScreen the player's
 * render state from the case's `figure` ({bbHeight, scale, fallFlying}) instead of a skinned avatar.
 */
final class RecipeBookStubs {
    private RecipeBookStubs() {}

    static final class NoTextInput extends com.mojang.blaze3d.platform.TextInputManager {
        NoTextInput() { super(null); }
        @Override public void setTextInputArea(int x0, int y0, int x1, int y1) { }
    }

    static final class Figure extends EntityRenderer<Entity, LivingEntityRenderState> {
        JsonObject figure;

        private Figure() { super(null); }

        @Override public LivingEntityRenderState createRenderState() { return new LivingEntityRenderState(); }

        @Override public void extractRenderState(Entity entity, LivingEntityRenderState state, float partial) {
            state.entityType = net.minecraft.core.registries.BuiltInRegistries.ENTITY_TYPE.getValue(net.minecraft.resources.Identifier.withDefaultNamespace("player"));
            state.boundingBoxWidth = 0.6F;
            state.boundingBoxHeight = figure.get("bbHeight").getAsFloat();
            state.scale = figure.get("scale").getAsFloat();
            state.pose = figure.get("fallFlying").getAsBoolean() ? Pose.FALL_FLYING : Pose.STANDING;
        }

        @Override protected void finalizeRenderState(Entity entity, LivingEntityRenderState state) { }
    }

    static final class Dispatcher extends EntityRenderDispatcher {
        Figure renderer;

        private Dispatcher() { super(null, null, null, null, null, null, null, null, null, null, null); }

        @SuppressWarnings("unchecked")
        @Override public <T extends Entity> EntityRenderer<? super T, ?> getRenderer(T entity) {
            return (EntityRenderer<? super T, ?>) (EntityRenderer<?, ?>) renderer;
        }
    }

    static void install(ScreenKit kit) throws Exception {
        Stub.put(kit.mc, Minecraft.class, "textInputManager", Stub.alloc(NoTextInput.class));
        var dispatcher = Stub.alloc(Dispatcher.class);
        dispatcher.renderer = Stub.alloc(Figure.class);
        dispatcher.renderer.figure = kit.c.has("figure") ? kit.c.getAsJsonObject("figure") : new JsonObject();
        Stub.put(kit.mc, Minecraft.class, "entityRenderDispatcher", dispatcher);
    }
}
