package acacia.parity.gui;

import static acacia.parity.gui.ScreenGame.b;
import static acacia.parity.gui.ScreenGame.component;
import static acacia.parity.gui.ScreenGame.i;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.mojang.serialization.Lifecycle;
import java.lang.reflect.Array;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.AlertScreen;
import net.minecraft.client.gui.screens.ConfirmScreen;
import net.minecraft.client.gui.screens.DeathScreen;
import net.minecraft.client.gui.screens.DisconnectedScreen;
import net.minecraft.client.gui.screens.GenericWaitingScreen;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.client.gui.screens.ProgressScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.social.PlayerSocialManager;
import net.minecraft.client.multiplayer.ClientCommonPacketListenerImpl;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.chat.report.ReportingContext;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.core.MappedRegistry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.chat.Component;
import net.minecraft.network.syncher.EntityDataAccessor;
import net.minecraft.network.syncher.SyncedDataHolder;
import net.minecraft.network.syncher.SynchedEntityData;
import net.minecraft.server.ServerLinks;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.player.Player;

/**
 * The `screens_menus` harness's oracle: the pause, death, confirm, alert, progress, disconnected
 * and completed-waiting screens built by `init` at the case's GUI size on the stand-in client
 * (in a world), ticked, and drawn as `extractRenderStateWithTooltipAndSubtitles` draws them
 * without the deferred tooltips. Case fields: cases.py.
 */
public final class MenusOracle {
    /** A player whose connection has no dialogs, server links or other players, with a score. */
    static LocalPlayer player(int score) throws Exception {
        var player = Stub.alloc(LocalPlayer.class);
        var connection = Stub.alloc(ClientPacketListener.class);
        var dialogs = new MappedRegistry<>(Registries.DIALOG, Lifecycle.stable());
        dialogs.freeze();
        Stub.put(connection, ClientPacketListener.class, "registryAccess", new RegistryAccess.ImmutableRegistryAccess(List.of(dialogs)).freeze());
        Stub.put(connection, ClientCommonPacketListenerImpl.class, "serverLinks", ServerLinks.EMPTY);
        Stub.put(connection, ClientCommonPacketListenerImpl.class, "seenPlayers", new java.util.HashMap<>());
        Stub.put(player, LocalPlayer.class, "connection", connection);
        @SuppressWarnings("unchecked")
        var id = (EntityDataAccessor<Integer>) Stub.field(Player.class, "DATA_SCORE_ID").get(null);
        var items = (SynchedEntityData.DataItem<?>[]) Array.newInstance(SynchedEntityData.DataItem.class, id.id() + 1);
        items[id.id()] = new SynchedEntityData.DataItem<>(id, score);
        var ctor = SynchedEntityData.class.getDeclaredConstructor(SyncedDataHolder.class, items.getClass());
        ctor.setAccessible(true);
        Stub.put(player, Entity.class, "entityData", ctor.newInstance(player, items));
        return player;
    }

    static Screen screen(Minecraft mc, JsonObject c) throws Exception {
        Component title = component(c.get("title"));
        return switch (c.get("kind").getAsString()) {
            case "pause" -> new PauseScreen(b(c, "showMenu", true));
            case "death" -> new DeathScreen(c.has("cause") ? component(c.get("cause")) : null, b(c, "hardcore", false), mc.player);
            case "confirm" -> b(c, "deathBackground", false)
                ? new DeathScreen.TitleConfirmScreen(v -> {}, title, component(c.get("message")), component(c.get("yes")), component(c.get("no")))
                : new ConfirmScreen(v -> {}, title, component(c.get("message")), component(c.get("yes")), component(c.get("no")));
            case "alert" -> new AlertScreen(() -> {}, title, component(c.get("message")), component(c.get("ok")), true);
            case "progress" -> {
                var p = new ProgressScreen(false);
                if (c.has("header")) p.progressStart(component(c.get("header")));
                if (c.has("stage")) p.progressStage(component(c.get("stage")));
                p.progressStagePercentage(i(c, "progress", 0));
                yield p;
            }
            case "disconnected" -> new DisconnectedScreen(null, title, component(c.get("message")));
            case "completed" -> GenericWaitingScreen.createCompleted(title, component(c.get("message")), component(c.get("button")), () -> {});
            default -> throw new IllegalArgumentException(c.get("kind").getAsString());
        };
    }

    public static void main(String[] args) throws Exception {
        ScreenGame.run(args, (c, result) -> {
            int width = i(c, "width", 427), height = i(c, "height", 240);
            var mc = ScreenGame.create(width, height, true);
            ScreenGame.set(mc.options.menuBackgroundBlurriness(), i(c, "blur", 5));
            Stub.put(mc, Minecraft.class, "playerSocialManager", Stub.alloc(PlayerSocialManager.class));
            Stub.put(mc, Minecraft.class, "reportingContext", Stub.alloc(ReportingContext.class));
            Stub.put(mc, Minecraft.class, "isLocalServer", b(c, "localServer", false));
            mc.player = player(i(c, "score", 0));
            var screen = screen(mc, c);
            screen.init(width, height);
            if (c.has("delay") && screen instanceof ConfirmScreen confirm) confirm.setDelay(i(c, "delay", 0));
            for (int t = 0; t < i(c, "ticks", 0); t++) screen.tick();
            if (b(c, "topmost", true)) Stub.put(mc.gui, net.minecraft.client.gui.Gui.class, "screen", screen);
            var capture = GuiCapture.create(mc, width, height);
            JsonArray mouse = c.getAsJsonArray("mouse");
            int mx = mouse.get(0).getAsInt(), my = mouse.get(1).getAsInt();
            capture.nextStratum();
            screen.extractBackground(capture, mx, my, 0.0F);
            capture.nextStratum();
            screen.extractRenderState(capture, mx, my, 0.0F);
            return capture.draws;
        });
    }
}
