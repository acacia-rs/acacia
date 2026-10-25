package acacia.parity.gui;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import net.minecraft.client.gui.navigation.ScreenRectangle;
import net.minecraft.client.gui.screens.inventory.tooltip.BelowOrAboveWidgetTooltipPositioner;
import net.minecraft.client.gui.screens.inventory.tooltip.ClientTooltipPositioner;
import net.minecraft.client.gui.screens.inventory.tooltip.DefaultTooltipPositioner;
import net.minecraft.client.gui.screens.inventory.tooltip.MenuTooltipPositioner;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.component.DataComponents;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.ComponentSerialization;
import net.minecraft.resources.Identifier;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.world.Difficulty;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.TooltipFlag;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.saveddata.maps.MapId;
import net.minecraft.world.level.saveddata.maps.MapItemSavedData;

/**
 * The `tooltips` harness's oracle. `item`: one stack's tooltip as a container screen sets it
 * (ItemStack.getTooltipLines with the case's advanced/creative flags, its tooltip image and
 * tooltip_style, extra space after the name) at the mouse; `lines`: plain component tooltips set
 * in order (each with a positioner, style and replace flag). Then extractDeferredElements, recorded
 * by GuiCapture. The stack's lines (plain) are state; the registry facts the Rust side reads come
 * from TooltipFacts.
 */
public final class TooltipOracle {
    static Component component(com.google.gson.JsonElement json) {
        return ComponentSerialization.CODEC.parse(ScreenKit.ops(), json).getOrThrow();
    }

    static ClientTooltipPositioner positioner(JsonObject t) {
        if (!t.has("positioner")) return DefaultTooltipPositioner.INSTANCE;
        var p = t.getAsJsonArray("positioner");
        var rect = new ScreenRectangle(p.get(1).getAsInt(), p.get(2).getAsInt(), p.get(3).getAsInt(), p.get(4).getAsInt());
        return switch (p.get(0).getAsString()) {
            case "below" -> new BelowOrAboveWidgetTooltipPositioner(rect);
            case "menu" -> new MenuTooltipPositioner(rect);
            default -> DefaultTooltipPositioner.INSTANCE;
        };
    }

    static Identifier style(JsonObject t) {
        return t.has("style") && !t.get("style").isJsonNull() ? Identifier.parse(t.get("style").getAsString()) : null;
    }

    /** The case's difficulty and map data on the level the tooltip context reads. */
    static void level(ScreenKit kit) throws Exception {
        var difficulty = kit.bool("peaceful", false) ? Difficulty.PEACEFUL : Difficulty.NORMAL;
        Stub.put(kit.level, ClientLevel.class, "clientLevelData", new ClientLevel.ClientLevelData(difficulty, false, false));
        @SuppressWarnings("unchecked")
        var maps = (java.util.Map<MapId, MapItemSavedData>) Stub.get(kit.level, ClientLevel.class, "mapData");
        for (var m : kit.list("maps")) {
            var a = m.getAsJsonArray();
            maps.put(new MapId(a.get(0).getAsInt()), MapItemSavedData.createForClient((byte) a.get(1).getAsInt(), a.get(2).getAsBoolean(), Level.OVERWORLD));
        }
    }

    static void item(ScreenKit kit, JsonObject c, int mx, int my) {
        ItemStack stack = kit.key(ScreenKit.stack(c.get("stack")), 0);
        var flag = new TooltipFlag.Default(kit.bool("advanced", false), kit.bool("creative", false));
        List<Component> lines = stack.getTooltipLines(Item.TooltipContext.of(kit.level), kit.player, flag);
        var plain = new JsonArray();
        lines.forEach(l -> plain.add(l.getString()));
        kit.result.add("lines", plain);
        kit.capture.setTooltipForNextFrame(kit.mc.font, lines, stack.getTooltipImage(), mx, my, stack.get(DataComponents.TOOLTIP_STYLE), true);
    }

    static void lines(ScreenKit kit) {
        for (var e : kit.list("tooltips")) {
            var t = e.getAsJsonObject();
            List<FormattedCharSequence> seqs = new ArrayList<>();
            t.getAsJsonArray("lines").forEach(l -> seqs.add(component(l).getVisualOrderText()));
            kit.capture.setTooltipForNextFrame(kit.mc.font, seqs, Optional.empty(), positioner(t), t.get("x").getAsInt(), t.get("y").getAsInt(),
                t.has("replace") && t.get("replace").getAsBoolean(), style(t));
        }
    }

    static void run(ScreenKit kit, JsonObject c) throws Exception {
        level(kit);
        int mx = c.getAsJsonArray("mouse").get(0).getAsInt();
        int my = c.getAsJsonArray("mouse").get(1).getAsInt();
        if (c.get("kind").getAsString().equals("lines")) lines(kit);
        else item(kit, c, mx, my);
        kit.capture.extractDeferredElements(mx, my, 0);
    }

    public static void main(String[] args) throws Exception {
        ScreenKit.run(args, TooltipOracle::run);
    }
}
