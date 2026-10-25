package acacia.parity.gui;

import static acacia.parity.gui.ScreenGame.b;
import static acacia.parity.gui.ScreenGame.component;
import static acacia.parity.gui.ScreenGame.i;
import static acacia.parity.gui.ScreenGame.s;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.mojang.authlib.GameProfile;
import com.mojang.authlib.minecraft.UserApiService;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.MultiLineEditBox;
import net.minecraft.client.gui.components.MultilineTextField;
import net.minecraft.client.gui.components.Whence;
import net.minecraft.client.gui.font.TextFieldHelper;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.inventory.AbstractSignEditScreen;
import net.minecraft.client.gui.screens.inventory.BookEditScreen;
import net.minecraft.client.gui.screens.inventory.BookViewScreen;
import net.minecraft.client.gui.screens.inventory.HangingSignEditScreen;
import net.minecraft.client.gui.screens.inventory.SignEditScreen;
import net.minecraft.client.gui.screens.telemetry.TelemetryEventWidget;
import net.minecraft.client.gui.screens.telemetry.TelemetryInfoScreen;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.server.network.Filterable;
import net.minecraft.util.Util;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.DyeColor;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.component.WritableBookContent;
import net.minecraft.world.level.block.entity.HangingSignBlockEntity;
import net.minecraft.world.level.block.entity.SignBlockEntity;
import net.minecraft.world.level.block.entity.SignText;
import net.minecraft.world.level.block.entity.SignTextSlot;

/**
 * The `screens_text` harness's oracle: the sign and hanging-sign editors, the book viewer and
 * editor and the telemetry info screen, built by `init` at the case's GUI size on the stand-in
 * client (in a world), put in the case's editing state, and drawn as
 * `extractRenderStateWithTooltipAndSubtitles` draws them without the deferred tooltips. The caret
 * blink start is pinned `blinkAgo` ms before now. Case fields: cases.py.
 */
public final class TextOracle {
    static void userFlags(Minecraft mc, boolean extra) throws Exception {
        var flags = new HashSet<UserApiService.UserFlag>();
        flags.add(UserApiService.UserFlag.TELEMETRY_ENABLED);
        if (extra) flags.add(UserApiService.UserFlag.OPTIONAL_TELEMETRY_AVAILABLE);
        Stub.put(mc, Minecraft.class, "userPropertiesFuture", CompletableFuture.completedFuture(new UserApiService.UserProperties(flags, Map.of())));
    }

    /**
     * Keeps the JIT from compiling or inlining `TelemetryInfoScreen`, whose code it would compile
     * with `EXTRA_TELEMETRY_AVAILABLE` folded to a constant, ignoring later writes (seen after ~300 cases).
     */
    static void interpretTelemetryScreen() throws Exception {
        String screen = "net/minecraft/client/gui/screens/telemetry/TelemetryInfoScreen.*";
        var file = java.nio.file.Files.createTempFile("telemetry-directives", ".json");
        java.nio.file.Files.writeString(file, "[{match: \"" + screen + "\", Exclude: true}, {match: \"*.*\", inline: \"-" + screen + "\"}]");
        var server = java.lang.management.ManagementFactory.getPlatformMBeanServer();
        var dcmd = new javax.management.ObjectName("com.sun.management:type=DiagnosticCommand");
        Object out = server.invoke(dcmd, "compilerDirectivesAdd", new Object[] {new String[] {file.toString()}}, new String[] {String[].class.getName()});
        if (String.valueOf(out).contains("rror")) throw new IllegalStateException("compiler directives: " + out);
    }

    /** A static final boolean written through Unsafe; see `interpretTelemetryScreen`. */
    static void putStaticBoolean(Class<?> owner, String name, boolean value) throws Exception {
        // A class literal does not run <clinit>, which would overwrite the write.
        Class.forName(owner.getName(), true, owner.getClassLoader());
        var f = Stub.field(owner, name);
        var unsafe = Stub.field(Class.forName("sun.misc.Unsafe"), "theUnsafe").get(null);
        var u = unsafe.getClass();
        Object base = u.getMethod("staticFieldBase", java.lang.reflect.Field.class).invoke(unsafe, f);
        long offset = (long) u.getMethod("staticFieldOffset", java.lang.reflect.Field.class).invoke(unsafe, f);
        u.getMethod("putBoolean", Object.class, long.class, boolean.class).invoke(unsafe, base, offset, value);
    }

    static Screen sign(JsonObject c) throws Exception {
        boolean hanging = b(c, "hanging", false), wall = b(c, "wall", false);
        String id = s(c, "wood", "oak") + (wall ? "_wall" : "") + (hanging ? "_hanging_sign" : "_sign");
        var state = BuiltInRegistries.BLOCK.getValue(Identifier.withDefaultNamespace(id)).defaultBlockState();
        SignBlockEntity sign = hanging ? new HangingSignBlockEntity(BlockPos.ZERO, state) : new SignBlockEntity(BlockPos.ZERO, state);
        var lines = new ArrayList<Component>();
        for (JsonElement e : c.getAsJsonArray("lines")) lines.add(Component.literal(e.getAsString()));
        var color = DyeColor.byName(s(c, "dye", "black"), DyeColor.BLACK);
        // setText would mark the (level-less) block updated.
        Stub.put(sign, SignBlockEntity.class, "frontText", new SignText(lines, lines, color, b(c, "glowing", false)));
        return hanging ? new HangingSignEditScreen(sign, SignTextSlot.FRONT, false) : new SignEditScreen(sign, SignTextSlot.FRONT, false);
    }

    static void signState(Screen screen, JsonObject c) throws Exception {
        Stub.put(screen, AbstractSignEditScreen.class, "line", i(c, "line", 0));
        var field = (TextFieldHelper) Stub.get(screen, AbstractSignEditScreen.class, "signField");
        Stub.put(field, TextFieldHelper.class, "cursorPos", i(c, "cursor", 0));
        Stub.put(field, TextFieldHelper.class, "selectionPos", i(c, "selection", 0));
        Stub.put(screen, AbstractSignEditScreen.class, "cursorBlinkStartTime", Util.getMillis() - i(c, "blinkAgo", 0));
    }

    static Screen bookView(JsonObject c) {
        var pages = new ArrayList<Component>();
        for (JsonElement e : c.getAsJsonArray("pages")) pages.add(component(e));
        return new BookViewScreen(new BookViewScreen.BookAccess(pages));
    }

    static Screen bookEdit(JsonObject c) throws Exception {
        var player = Stub.alloc(LocalPlayer.class);
        Stub.put(player, Player.class, "gameProfile", new GameProfile(new UUID(0, 1), "Steve"));
        var pages = new ArrayList<Filterable<String>>();
        for (JsonElement e : c.getAsJsonArray("pages")) pages.add(Filterable.passThrough(e.getAsString()));
        return new BookEditScreen(player, ItemStack.EMPTY, InteractionHand.MAIN_HAND, new WritableBookContent(pages));
    }

    /** Opens page `page` and runs the case's editing ops on the page box's field. */
    static void bookEditState(BookEditScreen screen, JsonObject c) throws Exception {
        if (i(c, "page", 0) != 0) {
            Stub.put(screen, BookEditScreen.class, "currentPage", i(c, "page", 0));
            Stub.method(BookEditScreen.class, "updatePageContent").invoke(screen);
            Stub.method(BookEditScreen.class, "updateButtonVisibility").invoke(screen);
        }
        var box = (MultiLineEditBox) Stub.get(screen, BookEditScreen.class, "page");
        var field = (MultilineTextField) Stub.get(box, MultiLineEditBox.class, "textField");
        for (JsonElement e : c.getAsJsonArray("ops")) {
            JsonArray op = e.getAsJsonArray();
            switch (op.get(0).getAsString()) {
                case "seek" -> {
                    field.setSelecting(op.get(2).getAsBoolean());
                    field.seekCursor(Whence.ABSOLUTE, op.get(1).getAsInt());
                }
                case "insert" -> field.insertText(op.get(1).getAsString());
                case "delete" -> field.deleteText(op.get(1).getAsInt());
                case "line" -> field.seekCursorLine(op.get(1).getAsInt());
                case "point" -> field.seekCursorToPoint(op.get(1).getAsDouble(), op.get(2).getAsDouble());
                default -> throw new IllegalArgumentException(op.toString());
            }
            field.setSelecting(false);
        }
        if (!b(c, "focused", true)) screen.clearFocus();
        Stub.put(box, MultiLineEditBox.class, "focusedTime", Util.getMillis() - i(c, "blinkAgo", 0));
    }

    public static void main(String[] args) throws Exception {
        interpretTelemetryScreen();
        ScreenGame.run(args, (c, result) -> {
            int width = i(c, "width", 427), height = i(c, "height", 240);
            var mc = ScreenGame.create(width, height, true);
            ScreenGame.set(mc.options.menuBackgroundBlurriness(), i(c, "blur", 5));
            boolean extra = b(c, "extra", false);
            userFlags(mc, extra);
            String kind = c.get("kind").getAsString();
            Screen screen = switch (kind) {
                case "sign" -> sign(c);
                case "book_view" -> bookView(c);
                case "book_edit" -> bookEdit(c);
                case "telemetry" -> {
                    putStaticBoolean(TelemetryInfoScreen.class, "EXTRA_TELEMETRY_AVAILABLE", extra);
                    ScreenGame.set(mc.options.telemetryOptInExtra(), b(c, "optIn", false));
                    yield new TelemetryInfoScreen(null, mc.options);
                }
                default -> throw new IllegalArgumentException(kind);
            };
            screen.init(width, height);
            switch (screen) {
                case AbstractSignEditScreen sign -> signState(sign, c);
                case BookViewScreen view -> { if (i(c, "page", 0) != 0) view.setPage(i(c, "page", 0)); }
                case BookEditScreen edit -> bookEditState(edit, c);
                case TelemetryInfoScreen t -> {
                    var events = (TelemetryEventWidget) Stub.get(t, TelemetryInfoScreen.class, "telemetryEventWidget");
                    events.setScrollAmount(c.get("scroll").getAsDouble());
                    if (!b(c, "focused", true)) t.clearFocus();
                }
                default -> { }
            }
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
