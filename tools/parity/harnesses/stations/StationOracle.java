package acacia.parity.gui;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.util.concurrent.ConcurrentLinkedQueue;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.components.WidgetTooltipHolder;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.client.gui.screens.inventory.AnvilScreen;
import net.minecraft.client.multiplayer.ClientCommonPacketListenerImpl;
import net.minecraft.client.renderer.MapRenderer;
import net.minecraft.client.renderer.state.MapRenderState;
import net.minecraft.network.Connection;
import net.minecraft.util.Util;
import net.minecraft.world.entity.player.Abilities;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.AbstractContainerMenu;
import net.minecraft.world.inventory.DataSlot;
import net.minecraft.world.inventory.Slot;
import net.minecraft.world.level.saveddata.maps.MapId;
import net.minecraft.world.level.saveddata.maps.MapItemSavedData;

/**
 * The `stations` harness's oracle: a station screen (StationScreens) over its real client-side
 * menu, the case's slots set in index order (the menu computes what it computes client-side, as
 * the anvil's and grindstone's results), the case's server-synced data slots, then one frame.
 * Every slot's final stack is keyed by its index; every data slot's final value goes to the probe
 * (`game.data`). Stand-ins: sends are queued on a never-connected Connection, text input areas and
 * the map texture are ignored, the partial tick is the case's.
 */
public final class StationOracle {
    private StationOracle() {}

    public static final int CARRIED = -2;

    static net.minecraft.network.chat.Component title(ScreenKit kit, net.minecraft.network.chat.Component fallback) {
        if (!kit.c.has("title") || kit.c.get("title").isJsonNull()) return fallback;
        return net.minecraft.network.chat.ComponentSerialization.CODEC.parse(ScreenKit.ops(), kit.c.get("title")).getOrThrow();
    }

    public static final class TextInput extends com.mojang.blaze3d.platform.TextInputManager {
        private TextInput() {
            super(null);
        }

        @Override public void setTextInputArea(int x0, int y0, int x1, int y1) { }

        @Override public void onTextInputFocusChange(Object owner, boolean focused) { }
    }

    /** Map textures are not prepared; decorations are carried over as they are. */
    public static final class Maps extends MapRenderer {
        private Maps() {
            super(null, null);
        }

        @Override public void extractRenderState(MapId id, MapItemSavedData data, MapRenderState state) {
            state.decorations.clear();
            data.getDecorations().forEach(d -> state.decorations.add(new MapRenderState.MapDecorationRenderState()));
        }
    }

    static void stubs(ScreenKit kit) throws Exception {
        var connection = Stub.alloc(Connection.class);
        Stub.put(connection, Connection.class, "pendingActions", new ConcurrentLinkedQueue<>());
        Stub.put(Stub.get(kit.player, net.minecraft.client.player.LocalPlayer.class, "connection"), ClientCommonPacketListenerImpl.class, "connection", connection);
        Stub.put(kit.mc, Minecraft.class, "textInputManager", Stub.alloc(TextInput.class));
        Stub.put(kit.mc, Minecraft.class, "mapRenderer", Stub.alloc(Maps.class));
        var timer = Stub.alloc(DeltaTracker.Timer.class);
        Stub.put(timer, DeltaTracker.Timer.class, "deltaTickResidual", kit.f("partial", 0));
        Stub.put(kit.mc, Minecraft.class, "deltaTracker", timer);
        Stub.put(kit.mc, Minecraft.class, "font", StationFont.get());
        kit.capture.font = StationFont.get();
        var abilities = new Abilities();
        abilities.instabuild = kit.bool("creative", false);
        Stub.put(kit.player, Player.class, "abilities", abilities);
        kit.player.experienceLevel = kit.i("xp", 0);
        for (var e : kit.list("maps")) {
            var a = e.getAsJsonArray();
            var data = MapItemSavedData.createForClient((byte) a.get(1).getAsInt(), a.get(2).getAsBoolean(), net.minecraft.world.level.Level.OVERWORLD);
            kit.level.overrideMapData(new MapId(a.get(0).getAsInt()), data);        }
    }

    /** The case's slots in index order, then its data slots ([index, value]). */
    static void fill(ScreenKit kit, AbstractContainerMenu menu) {
        for (var e : kit.list("slots")) {
            var a = e.getAsJsonArray();
            int index = a.get(0).getAsInt();
            if (index >= menu.slots.size()) continue;
            Slot slot = menu.slots.get(index);
            slot.container.setItem(slot.getContainerSlot(), ScreenKit.stack(a.get(1)));
        }
        menu.setCarried(ScreenKit.stack(kit.c.get("carried")));
        for (var e : kit.list("data")) {
            var a = e.getAsJsonArray();
            menu.setData(a.get(0).getAsInt(), a.get(1).getAsInt());
        }
    }

    /** The anvil's name box as the client leaves it: the input's name (or the case's), focused. */
    static void anvil(ScreenKit kit, AnvilScreen screen) throws Exception {
        var menu = screen.getMenu();
        var name = (EditBox) Stub.get(screen, AnvilScreen.class, "name");
        screen.slotChanged(menu, 0, menu.getSlot(0).getItem());
        if (kit.c.has("name") && menu.getSlot(0).hasItem()) name.setValue(kit.c.get("name").getAsString());
        screen.setFocused(name);
        // The cursor blinks every 300 ms from focusing; centred in its on or off phase.
        Stub.put(name, EditBox.class, "focusedTime", Util.getMillis() - (kit.bool("blink", false) ? 450 : 150));
    }

    /** As if the mouse has rested where it is: a hovered widget's tooltip shows at once. */
    static void settleTooltips(Screen screen) throws Exception {
        for (var child : screen.children()) {
            if (!(child instanceof AbstractWidget widget)) continue;
            var holder = (WidgetTooltipHolder) Stub.get(widget, AbstractWidget.class, "tooltip");
            Stub.put(holder, WidgetTooltipHolder.class, "wasDisplayed", true);
            Stub.put(holder, WidgetTooltipHolder.class, "displayStartTime", 0L);
        }
    }

    static void keyAll(ScreenKit kit, AbstractContainerMenu menu) throws Exception {
        for (int i = 0; i < menu.slots.size(); i++) kit.key(menu.slots.get(i).getItem(), i);
        kit.key(menu.getCarried(), CARRIED);
        @SuppressWarnings("unchecked")
        var slots = (java.util.List<DataSlot>) Stub.get(menu, AbstractContainerMenu.class, "dataSlots");
        var data = new JsonArray();
        slots.forEach(s -> data.add(s.get()));
        kit.game.add("data", data);
    }

    static void run(ScreenKit kit, JsonObject c) throws Exception {
        stubs(kit);
        String kind = c.get("kind").getAsString();
        AbstractContainerScreen<?> screen = kit.open(StationScreens.create(kit, kind));
        fill(kit, screen.getMenu());
        StationScreens.prepare(kit, screen);
        if (screen instanceof AnvilScreen anvil) anvil(kit, anvil);
        settleTooltips(screen);
        keyAll(kit, screen.getMenu());
        kit.frame(screen);
    }

    public static void main(String[] args) throws Exception {
        ScreenKit.run(args, StationOracle::run);
    }
}
