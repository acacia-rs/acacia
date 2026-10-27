package acacia.parity.gui;

import com.google.gson.JsonObject;
import com.mojang.serialization.JsonOps;
import java.util.ArrayDeque;
import java.util.ArrayList;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.gui.Hud;
import net.minecraft.client.gui.components.ChatComponent;
import net.minecraft.client.multiplayer.chat.ChatListener;
import net.minecraft.client.multiplayer.chat.GuiMessageTag;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.network.chat.ComponentSerialization;

/**
 * The chat the chat screen draws above its input: a real ChatComponent holding the case's
 * "messages" (component JSON, a tag, the tick each arrived), its options from the case, and
 * `advanced` item tooltips (show_entity hovers need them). Set up as HudOverlays.chat does.
 */
final class ChatLines {
    private ChatLines() {}

    static final String[] OPTIONS = {"chatScale", "chatWidth", "chatOpacity", "chatLineSpacing", "chatHeightUnfocused", "chatHeightFocused"};

    /** The level and player show_item tooltips read (Screen.getTooltipFromItem), as ScreenKit sets them up. */
    static void world(Minecraft mc, net.minecraft.client.multiplayer.ClientPacketListener connection, LocalPlayer player) throws Exception {
        var level = Stub.alloc(net.minecraft.client.multiplayer.ClientLevel.class);
        Stub.put(level, net.minecraft.world.level.Level.class, "registryAccess", GuiGame.access);
        Stub.put(level, net.minecraft.client.multiplayer.ClientLevel.class, "connection", connection);
        Stub.put(connection, net.minecraft.client.multiplayer.ClientPacketListener.class, "enabledFeatures", net.minecraft.world.flag.FeatureFlags.DEFAULT_FLAGS);
        Stub.put(mc, Minecraft.class, "level", level);
        Stub.put(player, net.minecraft.world.entity.Entity.class, "level", level);
        Stub.put(player, LocalPlayer.class, "permissions", net.minecraft.server.permissions.PermissionSet.NO_PERMISSIONS);
        Stub.put(player, net.minecraft.world.entity.LivingEntity.class, "attributes",
            new net.minecraft.world.entity.ai.attributes.AttributeMap(net.minecraft.world.entity.player.Player.createAttributes().build()));
    }

    static void setup(Minecraft mc, net.minecraft.client.gui.Gui gui, Hud hud, ChatComponent chat, Options options, JsonObject c) throws Exception {
        var C = ChatComponent.class;
        Stub.put(chat, C, "minecraft", mc);
        Stub.put(chat, C, "allMessages", new ArrayList<>());
        Stub.put(chat, C, "trimmedMessages", new ArrayList<>());
        Stub.put(chat, C, "messageDeletionQueue", new ArrayList<>());
        Stub.put(chat, C, "visibleMessageFilter", (java.util.function.Predicate<?>) m -> true);
        var listener = Stub.alloc(ChatListener.class);
        Stub.put(listener, ChatListener.class, "delayedMessageQueue", new ArrayDeque<>());
        Stub.put(gui, net.minecraft.client.gui.Gui.class, "chatListener", listener);
        options.advancedItemTooltips = c.has("advanced") && c.get("advanced").getAsBoolean();
        if (!c.has("messages")) return;
        for (String o : OPTIONS) Stub.put(options, Options.class, o, ChatScreenOracle.option(c.get(o).getAsDouble()));
        Stub.put(options, Options.class, "chatColors", ChatScreenOracle.option(true));
        var ops = GuiGame.lookup.createSerializationContext(JsonOps.INSTANCE);
        for (var e : c.getAsJsonArray("messages")) {
            var m = e.getAsJsonObject();
            Stub.put(hud, Hud.class, "tickCount", m.get("at").getAsInt());
            var text = ComponentSerialization.CODEC.parse(ops, m.get("text")).getOrThrow();
            switch (m.has("tag") && !m.get("tag").isJsonNull() ? m.get("tag").getAsString() : "") {
                case "system" -> chat.addClientSystemMessage(text);
                case "error" -> chat.addPlayerMessage(text, null, GuiMessageTag.chatError());
                case "not_secure" -> chat.addPlayerMessage(text, null, GuiMessageTag.chatNotSecure());
                case "modified" -> chat.addPlayerMessage(text, null, GuiMessageTag.chatModified(m.get("original").getAsString()));
                default -> chat.addPlayerMessage(text, null, null);
            }
        }
        Stub.put(hud, Hud.class, "tickCount", c.get("now").getAsInt());
    }
}
