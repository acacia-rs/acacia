package acacia.parity.gui;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import net.minecraft.client.gui.screens.inventory.BookViewScreen;
import net.minecraft.client.gui.screens.inventory.LecternScreen;
import net.minecraft.network.chat.ComponentSerialization;
import net.minecraft.world.entity.player.Abilities;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.LecternMenu;

/**
 * The `lectern` kind: LecternScreen over a lectern menu holding the case's `book` stack at page
 * `page`, the player may build or not (`build`). The probe gets the book's pages as components
 * (`game.pages`) and the page the screen settled on (`game.page`).
 */
final class Lectern {
    private Lectern() {}

    static void run(ScreenKit kit, JsonObject c) throws Exception {
        var abilities = new Abilities();
        abilities.mayBuild = kit.bool("build", true);
        Stub.put(kit.player, Player.class, "abilities", abilities);
        var menu = new LecternMenu(0);
        var screen = kit.open(new LecternScreen(menu, kit.inventory, net.minecraft.network.chat.Component.empty()));
        // As the server's packets arrive after the screen opened: setData broadcasts both to its listener.
        if (c.has("book")) menu.getSlot(0).set(kit.key(ScreenKit.stack(c.get("book")), 0));
        menu.setData(0, kit.i("page", 0));
        var access = c.has("book") ? BookViewScreen.BookAccess.fromItem(menu.getBook()) : null;
        var pages = new JsonArray();
        if (access != null) for (var p : access.pages()) pages.add(ComponentSerialization.CODEC.encodeStart(ScreenKit.ops(), p).getOrThrow());
        kit.game.add("pages", pages);
        kit.game.addProperty("page", (int) Stub.get(screen, BookViewScreen.class, "currentPage"));
        kit.frame(screen);
    }
}
