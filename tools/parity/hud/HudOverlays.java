package acacia.parity.gui;

import static acacia.parity.gui.HudActor.b;
import static acacia.parity.gui.HudActor.f;
import static acacia.parity.gui.HudActor.i;
import static acacia.parity.gui.HudKinds.comp;
import static acacia.parity.gui.HudOracle.option;

import com.google.gson.JsonObject;
import java.lang.reflect.Proxy;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Map;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.Hud;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.Entity;

/** The `hud` oracle's camera overlays, sleep overlay and chat kinds. */
final class HudOverlays {
    private HudOverlays() {}

    static void run(HudOracle.Ctx x, JsonObject c) throws Exception {
        Stub.put(x.options, Options.class, "vignette", option(b("vignette")));
        Stub.put(x.options, Options.class, "cameraType", b("firstPerson") ? net.minecraft.client.CameraType.FIRST_PERSON : net.minecraft.client.CameraType.THIRD_PERSON_BACK);
        Stub.put(x.options, Options.class, "screenEffectScale", option((double) f("screenEffectScale", 1)));
        var border = new net.minecraft.world.level.border.WorldBorder();
        if (c.has("border") && !c.get("border").isJsonNull()) {
            var bo = c.getAsJsonObject("border");
            border.setCenter(bo.get("centerX").getAsDouble(), bo.get("centerZ").getAsDouble());
            border.setSize(bo.get("size").getAsDouble());
            border.setWarningBlocks(bo.get("warningBlocks").getAsInt());
            border.setWarningTime(bo.get("warningTime").getAsInt());
        }
        Stub.put(x.level, ClientLevel.class, "worldBorder", border);
        Stub.put(x.mc, Minecraft.class, "level", x.level);
        Stub.put(x.actor, Entity.class, "position", new net.minecraft.world.phys.Vec3(f("camX", 0), 64, f("camZ", 0)));
        x.actor.oPortalEffectIntensity = c.getAsJsonArray("portal").get(0).getAsFloat();
        x.actor.portalEffectIntensity = c.getAsJsonArray("portal").get(1).getAsFloat();
        Stub.put(x.hud, Hud.class, "vignetteBrightness", f("vignetteBrightness", 1));
        Stub.put(x.hud, Hud.class, "scopeScale", f("scopeScale", 0.5F));
        // The nether portal's particle sprite, as the block model set would give it.
        var contents = Stub.alloc(net.minecraft.client.renderer.texture.SpriteContents.class);
        Stub.put(contents, net.minecraft.client.renderer.texture.SpriteContents.class, "name", Identifier.withDefaultNamespace("block/nether_portal"));
        var sprite = Stub.alloc(net.minecraft.client.renderer.texture.TextureAtlasSprite.class);
        Stub.put(sprite, net.minecraft.client.renderer.texture.TextureAtlasSprite.class, "contents", contents);
        var material = new net.minecraft.client.resources.model.sprite.Material.Baked(sprite, false);
        var model = (net.minecraft.client.renderer.block.dispatch.BlockStateModel) Proxy.newProxyInstance(HudOverlays.class.getClassLoader(),
            new Class<?>[] {net.minecraft.client.renderer.block.dispatch.BlockStateModel.class},
            (proxy, method, a) -> method.getName().equals("particleMaterial") ? material : null);
        var models = Stub.alloc(net.minecraft.client.resources.model.ModelManager.class);
        Stub.put(models, net.minecraft.client.resources.model.ModelManager.class, "blockStateModelSet",
            new net.minecraft.client.renderer.block.BlockStateModelSet(Map.of(), model));
        Stub.put(x.mc, Minecraft.class, "modelManager", models);
        var G = GuiGraphicsExtractor.class;
        HudKinds.hud("extractCameraOverlays", G, DeltaTracker.class).invoke(x.hud, x.capture, x.delta);
        HudKinds.hud("extractSleepOverlay", G, DeltaTracker.class).invoke(x.hud, x.capture, x.delta);
        x.result.addProperty("scopeScale", (float) Stub.get(x.hud, Hud.class, "scopeScale"));
    }

    static void chat(HudOracle.Ctx x, JsonObject c) throws Exception {
        for (String o : new String[] {"chatScale", "chatWidth", "chatOpacity", "chatLineSpacing", "chatHeightUnfocused", "chatHeightFocused"})
            Stub.put(x.options, Options.class, o, option(c.get(o).getAsDouble()));
        Stub.put(x.options, Options.class, "chatColors", option(true));
        Stub.putStatic(Minecraft.class, "instance", x.mc);
        Stub.put(x.gui, net.minecraft.client.gui.Gui.class, "hud", x.hud);
        var listener = Stub.alloc(net.minecraft.client.multiplayer.chat.ChatListener.class);
        Stub.put(listener, net.minecraft.client.multiplayer.chat.ChatListener.class, "delayedMessageQueue", new ArrayDeque<>());
        Stub.put(x.gui, net.minecraft.client.gui.Gui.class, "chatListener", listener);
        var C = net.minecraft.client.gui.components.ChatComponent.class;
        Stub.put(x.chat, C, "minecraft", x.mc);
        Stub.put(x.chat, C, "allMessages", new ArrayList<>());
        Stub.put(x.chat, C, "trimmedMessages", new ArrayList<>());
        Stub.put(x.chat, C, "visibleMessageFilter", (java.util.function.Predicate<?>) m -> true);
        for (var e : c.getAsJsonArray("messages")) {
            var m = e.getAsJsonObject();
            Stub.put(x.hud, Hud.class, "tickCount", m.get("at").getAsInt());
            var text = comp(m.get("text"));
            switch (m.get("tag").isJsonNull() ? "" : m.get("tag").getAsString()) {
                case "system" -> x.chat.addClientSystemMessage(text);
                case "error" -> x.chat.addPlayerMessage(text, null, net.minecraft.client.multiplayer.chat.GuiMessageTag.chatError());
                default -> x.chat.addPlayerMessage(text, null, null);
            }
        }
        // Focused: the chat screen is open, the chat drawn in the foreground and maybe scrolled.
        boolean focused = b("focused");
        if (focused) Stub.put(x.gui, net.minecraft.client.gui.Gui.class, "screen", Stub.alloc(net.minecraft.client.gui.screens.ChatScreen.class));
        if (c.has("scroll")) x.chat.scrollChat(i("scroll", 0));
        x.chat.extractRenderState(x.capture, GuiGame.font, i("now", 0), 0, 0, focused
            ? net.minecraft.client.gui.components.ChatComponent.DisplayMode.FOREGROUND
            : net.minecraft.client.gui.components.ChatComponent.DisplayMode.BACKGROUND, false);
        Stub.put(x.gui, net.minecraft.client.gui.Gui.class, "screen", null);
    }
}
