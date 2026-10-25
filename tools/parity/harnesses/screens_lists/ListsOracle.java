package acacia.parity.gui;

import static acacia.parity.gui.ScreenGame.b;
import static acacia.parity.gui.ScreenGame.component;
import static acacia.parity.gui.ScreenGame.i;
import static acacia.parity.gui.ScreenGame.s;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.mojang.blaze3d.platform.InputConstants;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.TreeMap;
import java.util.function.BiFunction;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.options.LanguageSelectScreen;
import net.minecraft.client.gui.screens.options.controls.KeyBindsScreen;
import net.minecraft.client.gui.screens.packs.PackSelectionModel;
import net.minecraft.client.gui.screens.packs.PackSelectionScreen;
import net.minecraft.client.gui.screens.packs.TransferableSelectionList;
import net.minecraft.client.input.InputQuirks;
import net.minecraft.client.resources.language.LanguageInfo;
import net.minecraft.client.resources.language.LanguageManager;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.server.packs.repository.PackCompatibility;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.PackSource;
import net.minecraft.util.Util;

/**
 * The `screens_lists` harness's oracle: KeyBindsScreen, LanguageSelectScreen and
 * PackSelectionScreen built by `init` on the stand-in client (in a world) from the case's state,
 * then drawn as `extractRenderStateWithTooltipAndSubtitles` draws without the deferred tooltips.
 * Stand-ins: the keyboard's OS key names are a US layout's (as the Rust side's
 * `keys_table::us_layout_char`; SDL is not initialised here); the language manager holds the case's
 * languages; the pack lists are filled with entries made from the case (the model's own repository
 * is empty), filtered as `filterEntries` filters. Case fields: cases.py.
 */
public final class ListsOracle {
    static String us(int sc) {
        if (sc >= 4 && sc <= 29) return String.valueOf((char) ('A' + sc - 4));
        if (sc >= 30 && sc <= 38) return String.valueOf((char) ('1' + sc - 30));
        return switch (sc) {
            case 39 -> "0"; case 45 -> "-"; case 46 -> "="; case 47 -> "["; case 48 -> "]"; case 49 -> "\\";
            case 51 -> ";"; case 52 -> "'"; case 53 -> "`"; case 54 -> ","; case 55 -> "."; case 56 -> "/";
            default -> null;
        };
    }

    /** InputConstants.Type.KEYBOARD's display text with `us` in place of SDL_GetKeyName. */
    static void usKeyboardNames() throws Exception {
        BiFunction<Integer, String, Component> names = (value, name) -> {
            if ("key.keyboard.unknown".equals(name)) return Component.translatable(name);
            String system = us(value);
            return system != null ? Component.literal(system.toUpperCase(Locale.ROOT)) : Component.translatable(InputQuirks.keyboardTranslationKey(name));
        };
        Stub.put(InputConstants.Type.KEYBOARD, InputConstants.Type.class, "displayTextSupplier", names);
    }

    static InputConstants.Key key(JsonArray k) {
        var type = k.get(0).getAsString().equals("mouse") ? InputConstants.Type.MOUSE : InputConstants.Type.KEYBOARD;
        return type.getOrCreate(k.get(1).getAsInt());
    }

    static KeyMapping mapping(Minecraft mc, String name) {
        for (KeyMapping m : mc.options.keyMappings) if (m.getName().equals(name)) return m;
        throw new IllegalArgumentException(name);
    }

    static Screen keybinds(Minecraft mc, JsonObject c, int w, int h) throws Exception {
        if (c.has("keys")) for (var e : c.getAsJsonObject("keys").entrySet()) mapping(mc, e.getKey()).setKey(key(e.getValue().getAsJsonArray()));
        KeyMapping.resetMapping();
        var screen = new KeyBindsScreen(null, mc.options);
        screen.init(w, h);
        var list = (net.minecraft.client.gui.components.AbstractScrollArea) Stub.get(screen, KeyBindsScreen.class, "keyBindsList");
        list.setScrollAmount(i(c, "scroll", 0));
        if (c.has("selected") && !c.get("selected").isJsonNull()) screen.selectedKey = mapping(mc, c.get("selected").getAsString());
        return screen;
    }

    static void focusTime(EditBox box, JsonObject c) throws Exception {
        Stub.put(box, EditBox.class, "focusedTime", Util.getMillis() - i(c, "focusAgo", 0));
    }

    static Screen language(Minecraft mc, JsonObject c, int w, int h) throws Exception {
        var manager = Stub.alloc(LanguageManager.class);
        var languages = new TreeMap<String, LanguageInfo>();
        for (JsonElement l : c.getAsJsonArray("languages")) {
            var a = l.getAsJsonArray();
            languages.put(a.get(0).getAsString(), new LanguageInfo(a.get(2).getAsString(), a.get(1).getAsString(), false));
        }
        Stub.put(manager, LanguageManager.class, "languages", languages);
        Stub.put(manager, LanguageManager.class, "currentCode", s(c, "selected", "en_us"));
        var screen = new LanguageSelectScreen(null, mc.options, manager);
        screen.init(w, h);
        var list = (net.minecraft.client.gui.components.AbstractSelectionList<?>) Stub.get(screen, LanguageSelectScreen.class, "languageSelectionList");
        if (c.has("scroll") && !c.get("scroll").isJsonNull()) list.setScrollAmount(c.get("scroll").getAsDouble());
        var search = (EditBox) Stub.get(screen, LanguageSelectScreen.class, "search");
        String text = s(c, "search", "");
        if (!text.isEmpty()) search.setValue(text);
        if (b(c, "listFocus", false)) {
            screen.setFocused(list);
            list.setFocused(list.getSelected());
        }
        focusTime(search, c);
        return screen;
    }

    /** A pack entry from the case. */
    record CaseEntry(JsonObject p, boolean selected) implements PackSelectionModel.Entry {
        @Override public Identifier getIconTexture() { return Identifier.withDefaultNamespace("textures/" + p.get("icon").getAsString() + ".png"); }
        @Override public PackCompatibility getCompatibility() { return PackCompatibility.valueOf(p.get("compat").getAsString()); }
        @Override public String getId() { return p.get("id").getAsString(); }
        @Override public Component getTitle() { return component(p.get("title")); }
        @Override public Component getDescription() { return component(p.get("description")); }
        @Override public Component getExtendedDescription() { return getDescription(); }
        @Override public PackSource getPackSource() { return PackSource.DEFAULT; }
        @Override public boolean isFixedPosition() { return b(p, "fixed", false); }
        @Override public boolean isRequired() { return b(p, "required", false); }
        @Override public void select() {}
        @Override public void unselect() {}
        @Override public void moveUp() {}
        @Override public void moveDown() {}
        @Override public boolean isSelected() { return selected; }
        @Override public boolean canMoveUp() { return b(p, "up", false); }
        @Override public boolean canMoveDown() { return b(p, "down", false); }
    }

    /** `filterEntries`' test. */
    static List<PackSelectionModel.Entry> filter(JsonArray packs, boolean selected, String value) {
        var out = new ArrayList<PackSelectionModel.Entry>();
        String lower = value.toLowerCase(Locale.ROOT);
        for (JsonElement e : packs) {
            var entry = new CaseEntry(e.getAsJsonObject(), selected);
            if (value.isBlank() || entry.getId().toLowerCase(Locale.ROOT).contains(lower) || entry.getTitle().getString().toLowerCase(Locale.ROOT).contains(lower)
                || entry.getDescription().getString().toLowerCase(Locale.ROOT).contains(lower)) out.add(entry);
        }
        return out;
    }

    static Screen packs(Minecraft mc, JsonObject c, int w, int h) throws Exception {
        var dir = Files.createTempDirectory("packs");
        var screen = new PackSelectionScreen(new PackRepository(), r -> {}, dir, component(c.get("title")));
        screen.init(w, h);
        var lists = new TransferableSelectionList[] {
            (TransferableSelectionList) Stub.get(screen, PackSelectionScreen.class, "availablePackList"),
            (TransferableSelectionList) Stub.get(screen, PackSelectionScreen.class, "selectedPackList")};
        var search = (EditBox) Stub.get(screen, PackSelectionScreen.class, "search");
        search.setResponder(v -> {});
        String text = s(c, "search", "");
        if (!text.isEmpty()) search.setValue(text);
        lists[0].updateList(filter(c.getAsJsonArray("available"), false, text).stream(), null);
        lists[1].updateList(filter(c.getAsJsonArray("selected"), true, text).stream(), null);
        JsonArray scroll = c.getAsJsonArray("scroll");
        for (int n = 0; n < 2; n++) lists[n].setScrollAmount(scroll.get(n).getAsInt());
        if (c.has("focus") && !c.get("focus").isJsonNull()) {
            var f = c.getAsJsonArray("focus");
            var list = lists[f.get(0).getAsInt()];
            int row = f.get(1).getAsInt() + 1;
            if (row < list.children().size()) {
                screen.setFocused(list);
                list.setFocused((GuiEventListener) list.children().get(row));
            }
        }
        focusTime(search, c);
        return screen;
    }

    static void draw(Screen screen, GuiCapture capture, JsonArray mouse) {
        int mx = mouse.get(0).getAsInt(), my = mouse.get(1).getAsInt();
        capture.nextStratum();
        screen.extractBackground(capture, mx, my, 0.0F);
        capture.nextStratum();
        screen.extractRenderState(capture, mx, my, 0.0F);
    }

    public static void main(String[] args) throws Exception {
        ScreenGame.run(args, (c, result) -> {
            usKeyboardNames();
            int width = i(c, "width", 427), height = i(c, "height", 240);
            var mc = ScreenGame.create(width, height, true);
            ScreenGame.set(mc.options.menuBackgroundBlurriness(), i(c, "blur", 5));
            Screen screen = switch (c.get("kind").getAsString()) {
                case "keybinds" -> keybinds(mc, c, width, height);
                case "language" -> language(mc, c, width, height);
                case "packs" -> packs(mc, c, width, height);
                default -> throw new IllegalArgumentException(c.get("kind").getAsString());
            };
            JsonArray mouse = c.getAsJsonArray("mouse");
            if (b(c, "warm", false)) draw(screen, GuiCapture.create(mc, width, height), mouse);
            var capture = GuiCapture.create(mc, width, height);
            draw(screen, capture, mouse);
            return capture.draws;
        });
    }
}
