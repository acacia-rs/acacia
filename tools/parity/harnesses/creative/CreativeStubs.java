package acacia.parity.gui;

import com.google.gson.JsonObject;
import com.mojang.blaze3d.platform.TextInputManager;
import java.util.HashMap;
import java.util.UUID;
import net.minecraft.client.HotbarManager;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.world.Difficulty;
import net.minecraft.world.level.Level;
import net.minecraft.client.multiplayer.SessionSearchTrees;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.client.player.inventory.Hotbar;
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
import net.minecraft.client.renderer.entity.player.AvatarRenderer;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityEquipment;
import net.minecraft.world.entity.EntityTypes;
import net.minecraft.world.entity.Pose;
import net.minecraft.world.entity.player.Abilities;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.entity.player.PlayerModelType;
import net.minecraft.world.inventory.InventoryMenu;

/**
 * What the creative screen needs of the client beyond ScreenKit: a creative player with its
 * inventory menu, the session's search trees, saved hotbars, a text input manager that does not
 * reach SDL, and an avatar renderer filling only the render state fields the player figure reads
 * (the case's `figure`: bounding box, scale, pose).
 */
final class CreativeStubs {
    private CreativeStubs() {}

    /** Shared across cases: the creative tabs are rebuilt (and these updated) only when the parameters change. */
    static final SessionSearchTrees TREES = new SessionSearchTrees();

    public static final class Figure extends AvatarRenderer<AbstractClientPlayer> {
        JsonObject figure;

        private Figure() {
            super(null, false);
        }

        @Override public AvatarRenderState createRenderState() { return new AvatarRenderState(); }

        @Override public void extractRenderState(AbstractClientPlayer entity, AvatarRenderState state, float partial) {
            state.entityType = EntityTypes.PLAYER;
            state.boundingBoxWidth = figure.get("bbWidth").getAsFloat();
            state.boundingBoxHeight = figure.get("bbHeight").getAsFloat();
            state.scale = figure.get("scale").getAsFloat();
            state.pose = figure.get("fallFlying").getAsBoolean() ? Pose.FALL_FLYING : Pose.STANDING;
        }

        @Override protected void finalizeRenderState(AbstractClientPlayer entity, AvatarRenderState state) { }
    }

    public static final class NoInput extends TextInputManager {
        private NoInput() {
            super(null);
        }

        @Override public void setTextInputArea(int x0, int y0, int x1, int y1) { }
        @Override public void startTextInput(Object owner) { }
        @Override public void stopTextInput() { }
    }

    static void install(ScreenKit kit) throws Exception {
        var mc = kit.mc;
        var player = kit.player;
        var abilities = new Abilities();
        abilities.instabuild = true;
        Stub.put(player, Player.class, "abilities", abilities);
        Stub.put(player, Entity.class, "uuid", new UUID(0x1234L, 0x5678L));
        Stub.put(player, Player.class, "inventoryMenu", new InventoryMenu(kit.inventory, false, player));
        Stub.put(player, Player.class, "containerMenu", player.inventoryMenu);
        Stub.put(player.connection, ClientPacketListener.class, "searchTrees", TREES);
        Stub.put(mc, Minecraft.class, "textInputManager", Stub.alloc(NoInput.class));
        var levelData = new ClientLevel.ClientLevelData(Difficulty.NORMAL, false, false);
        Stub.put(kit.level, Level.class, "levelData", levelData);
        Stub.put(kit.level, ClientLevel.class, "clientLevelData", levelData);
        kit.applyEffects();

        var figure = Stub.alloc(Figure.class);
        figure.figure = kit.c.has("figure") ? kit.c.getAsJsonObject("figure") : new JsonObject();
        var dispatcher = Stub.alloc(EntityRenderDispatcher.class);
        var players = new HashMap<PlayerModelType, AvatarRenderer<AbstractClientPlayer>>();
        for (var model : PlayerModelType.values()) players.put(model, figure);
        Stub.put(dispatcher, EntityRenderDispatcher.class, "playerRenderers", players);
        Stub.put(mc, Minecraft.class, "entityRenderDispatcher", dispatcher);

        var hotbars = Stub.alloc(HotbarManager.class);
        var saved = new Hotbar[9];
        var rows = kit.list("hotbars");
        for (int i = 0; i < 9; i++) {
            saved[i] = new Hotbar();
            if (i >= rows.size() || !rows.get(i).isJsonArray()) continue;
            var inventory = new Inventory(player, new EntityEquipment());
            var row = rows.get(i).getAsJsonArray();
            for (int j = 0; j < 9 && j < row.size(); j++) inventory.setItem(j, ScreenKit.stack(row.get(j)));
            saved[i].storeFrom(inventory, GuiGame.access);
        }
        Stub.put(hotbars, HotbarManager.class, "hotbars", saved);
        Stub.put(hotbars, HotbarManager.class, "loaded", true);
        Stub.put(mc, Minecraft.class, "hotbarManager", hotbars);
    }
}
