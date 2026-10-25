package acacia.parity.gui;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.lang.reflect.InvocationTargetException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import net.minecraft.client.InputType;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.Hud;
import net.minecraft.client.gui.components.ChatComponent;
import net.minecraft.client.gui.components.CommandSuggestions;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.components.Renderable;
import net.minecraft.client.gui.navigation.ScreenRectangle;
import net.minecraft.client.gui.screens.ChatScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.CharacterEvent;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.client.input.MouseButtonInfo;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.ClientSuggestionProvider;
import net.minecraft.client.multiplayer.chat.ChatAbilities;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.network.chat.Component;
import net.minecraft.util.ArrayListDeque;
import net.minecraft.util.Util;

/**
 * The `chat_screen` harness's oracle: a real ChatScreen (its EditBox and CommandSuggestions)
 * driven by the case's typed text, keys, clicks and scrolls, then drawn: the input's background,
 * the chat's lines with the mouse over them (ChatLines), the EditBox, the suggestions, then the
 * deferred hover tooltip. Names complete to the case's online players, commands to the case's
 * tree (ChatCommands). Input lines: case JSON (cases.py);
 * output lines {case, value, cursor, highlight, display, draws}, {case, closed, draws} or
 * {case, error}. Adapted from Horizon's VanillaChatScreenOracle and VanillaPackScreenOracle (MIT,
 * (c) Ludovic Marie).
 */
public final class ChatScreenOracle {
    private ChatScreenOracle() {}

    static List<String> players = List.of();
    static ClientPacketListener connection;
    static net.minecraft.client.MouseHandler mouseHandler;

    static <T> net.minecraft.client.OptionInstance<T> option(T value) throws Exception {
        @SuppressWarnings("unchecked") net.minecraft.client.OptionInstance<T> o = Stub.alloc(net.minecraft.client.OptionInstance.class);
        Stub.put(o, net.minecraft.client.OptionInstance.class, "value", value);
        return o;
    }

    /** The client's suggestion provider: a message completes to the online players. */
    public static final class Provider extends ClientSuggestionProvider {
        private Provider() { super(null, null, null); }
        @Override public Collection<String> getOnlinePlayerNames() { return new ArrayList<>(players); }
        @Override public Collection<String> getCustomTabSuggestions() { return getOnlinePlayerNames(); }
    }

    /** Sent messages only. */
    public static final class Chat extends ChatComponent {
        private Chat() { super(null); }
        @Override public void resetChatScroll() { }
        @Override public void discardDraft() { }
        @Override public void saveAsDraft(String text) { }
        @Override public void captureClickableText(net.minecraft.client.gui.ActiveTextCollector output, int screenHeight, int ticks,
                                                   ChatComponent.DisplayMode mode) { }
    }

    public static final class NoNarrator extends net.minecraft.client.GameNarrator {
        private NoNarrator() { super(null); }
        @Override public void saySystemNow(Component message) { }
        @Override public void saySystemNow(String message) { }
    }

    static final class NoTextInput extends com.mojang.blaze3d.platform.TextInputManager {
        NoTextInput() { super(null); }
        @Override public void setTextInputArea(int x0, int y0, int x1, int y1) { }
        @Override public void onTextInputFocusChange(Object owner, boolean focused) { }
        @Override public void startTextInput(Object owner) { }
        @Override public void stopTextInput(Object owner) { }
        @Override public void stopTextInput() { }
    }

    public static final class NoSound extends net.minecraft.client.sounds.SoundManager {
        private NoSound() { super(null); }
        @Override public net.minecraft.client.sounds.SoundEngine.PlayResult play(net.minecraft.client.resources.sounds.SoundInstance instance) { return null; }
    }

    public static final class NoKeyboard extends net.minecraft.client.KeyboardHandler {
        private NoKeyboard() { super(null); }
        @Override public void resubmitLastPreeditEvent(net.minecraft.client.gui.components.events.GuiEventListener e) { }
    }

    /** Gui.setScreen without the window: the screen shown, the old one removed. */
    public static final class StubGui extends net.minecraft.client.gui.Gui {
        Screen current;
        int width, height;
        private StubGui() { super(null, null, null); }
        @Override public void setScreen(Screen screen) {
            try {
                if (current != null) {
                    current.clearFocus();
                    current.removed();
                }
                current = screen;
                if (screen == null) return;
                screen.added();
                screen.width = width;
                screen.height = height;
                Stub.method(Screen.class, "init").invoke(screen);
                Stub.method(Screen.class, "setInitialFocus").invoke(screen);
                Stub.put(screen, Screen.class, "initialized", true);
            } catch (Exception e) {
                throw new RuntimeException(e);
            }
        }
        @Override public Screen screen() { return current; }
    }

    /** A key by name: [SDL scancode, keycode], after "shift+" (1) and "ctrl+" (64) modifiers. */
    static KeyEvent key(String name) {
        int mods = 0;
        while (true) {
            if (name.startsWith("shift+")) { mods |= 1; name = name.substring(6); }
            else if (name.startsWith("ctrl+")) { mods |= 64; name = name.substring(5); }
            else break;
        }
        int[] k = switch (name) {
            case "backspace" -> new int[] {42, 8};
            case "delete" -> new int[] {76, 127};
            case "home" -> new int[] {74, 1073741898};
            case "end" -> new int[] {77, 1073741901};
            case "right" -> new int[] {79, 1073741903};
            case "left" -> new int[] {80, 1073741904};
            case "down" -> new int[] {81, 1073741905};
            case "up" -> new int[] {82, 1073741906};
            case "tab" -> new int[] {43, 9};
            case "escape" -> new int[] {41, 27};
            case "a" -> new int[] {4, 97};
            default -> throw new IllegalArgumentException(name);
        };
        return new KeyEvent(k[0], k[1], mods);
    }

    static Minecraft client(StubGui gui) throws Exception {
        Minecraft mc = Stub.alloc(Minecraft.class);
        Stub.put(mc, Minecraft.class, "font", GuiGame.font);
        Stub.put(mc, Minecraft.class, "fontFilterFishy", GuiGame.font);
        Stub.put(mc, Minecraft.class, "lastInputType", InputType.MOUSE);
        Stub.put(mc, Minecraft.class, "textInputManager", new NoTextInput());
        Stub.put(mc, Minecraft.class, "soundManager", Stub.alloc(NoSound.class));
        Stub.put(mc, Minecraft.class, "keyboardHandler", Stub.alloc(NoKeyboard.class));
        Stub.put(mc, Minecraft.class, "narrator", Stub.alloc(NoNarrator.class));
        Stub.put(mc, Minecraft.class, "gui", gui);
        connection = Stub.alloc(ClientPacketListener.class);
        mouseHandler = Stub.alloc(net.minecraft.client.MouseHandler.class);
        Stub.put(mc, Minecraft.class, "mouseHandler", mouseHandler);
        var provider = Stub.alloc(Provider.class);
        Stub.put(provider, ClientSuggestionProvider.class, "minecraft", mc);
        Stub.put(connection, ClientPacketListener.class, "suggestionsProvider", provider);
        var player = Stub.alloc(LocalPlayer.class);
        Stub.put(player, LocalPlayer.class, "connection", connection);
        Stub.put(player, LocalPlayer.class, "chatAbilities", ChatAbilities.NO_RESTRICTIONS);
        ChatLines.world(mc, connection, player);
        Stub.put(mc, Minecraft.class, "player", player);
        Stub.putStatic(Minecraft.class, "instance", mc);
        return mc;
    }

    static JsonObject run(JsonObject c, Minecraft mc, StubGui gui) throws Exception {
        int width = c.get("width").getAsInt(), height = c.get("height").getAsInt();
        gui.width = width;
        gui.height = height;
        gui.current = null;
        players = new ArrayList<>();
        for (var p : c.getAsJsonArray("players")) players.add(p.getAsString());
        var chat = Stub.alloc(Chat.class);
        var recent = new ArrayListDeque<String>(100);
        for (var s : c.getAsJsonArray("history")) recent.add(s.getAsString());
        Stub.put(chat, ChatComponent.class, "recentChat", recent);
        var hud = Stub.alloc(Hud.class);
        Stub.put(hud, Hud.class, "chat", chat);
        Stub.put(gui, net.minecraft.client.gui.Gui.class, "hud", hud);
        var options = Stub.alloc(Options.class);
        Stub.put(options, Options.class, "autoSuggestions", option(c.get("autoSuggest").getAsBoolean()));
        Stub.put(options, Options.class, "backgroundForChatOnly", option(c.get("chatOnly").getAsBoolean()));
        Stub.put(options, Options.class, "textBackgroundOpacity", option(c.get("opacity").getAsDouble()));
        Stub.put(options, Options.class, "saveChatDrafts", option(false));
        var window = Stub.alloc(com.mojang.blaze3d.platform.Window.class);
        Stub.put(window, com.mojang.blaze3d.platform.Window.class, "guiScaledWidth", width);
        Stub.put(window, com.mojang.blaze3d.platform.Window.class, "guiScaledHeight", height);
        Stub.put(window, com.mojang.blaze3d.platform.Window.class, "width", width);
        Stub.put(window, com.mojang.blaze3d.platform.Window.class, "height", height);
        Stub.put(connection, ClientPacketListener.class, "commands", ChatCommands.dispatcher(c));
        int mx = c.getAsJsonArray("mouse").get(0).getAsInt(), my = c.getAsJsonArray("mouse").get(1).getAsInt();
        Stub.put(mouseHandler, net.minecraft.client.MouseHandler.class, "xpos", (double) mx);
        Stub.put(mouseHandler, net.minecraft.client.MouseHandler.class, "ypos", (double) my);
        Stub.put(mc, Minecraft.class, "window", window);
        Stub.put(mc, Minecraft.class, "options", options);
        ChatLines.setup(mc, gui, hud, chat, options, c);
        var screen = new ChatScreen(c.get("initial").getAsString(), false);
        gui.setScreen(screen);
        if (c.has("scroll")) chat.scrollChat(c.get("scroll").getAsInt());
        var info = new MouseButtonInfo(1, 0);
        for (var a : c.getAsJsonArray("actions")) {
            var o = a.getAsJsonObject();
            if (gui.current != screen) break;
            switch (o.get("type").getAsString()) {
                case "type" -> o.get("text").getAsString().codePoints().forEach(cp -> screen.charTyped(new CharacterEvent(cp)));
                case "key" -> screen.keyPressed(key(o.get("key").getAsString()));
                case "click" -> {
                    var event = new MouseButtonEvent(o.get("x").getAsDouble(), o.get("y").getAsDouble(), info);
                    screen.mouseClicked(event, false);
                    screen.mouseReleased(event);
                }
                // The list's half of ChatScreen.mouseScrolled; the chat lines' half reads the keyboard.
                case "scroll" -> ((CommandSuggestions) Stub.get(screen, ChatScreen.class, "commandSuggestions")).mouseScrolled(o.get("amount").getAsDouble());
                default -> throw new IllegalArgumentException(o.toString());
            }
        }
        var result = new JsonObject();
        if (gui.current != screen) {
            result.addProperty("closed", true);
            result.add("draws", new JsonArray());
            return result;
        }
        var input = (EditBox) Stub.get(screen, ChatScreen.class, "input");
        Stub.put(input, EditBox.class, "focusedTime", Util.getMillis() - c.get("blink").getAsLong());
        var g = GuiCapture.create(mc, width, height);
        var scissors = Class.forName("net.minecraft.client.gui.GuiGraphicsExtractor$ScissorStack").getDeclaredConstructor(ScreenRectangle.class);
        scissors.setAccessible(true);
        Stub.put(g, GuiGraphicsExtractor.class, "scissorStack", scissors.newInstance(new ScreenRectangle(0, 0, width, height)));
        // ChatScreen.extractRenderState (insertionClickMode reads the keyboard: false), then the deferred tooltip.
        g.scanHover = true;
        g.hoverX = mx;
        g.hoverY = my;
        g.fill(2, height - 14, width - 2, height - 2, options.getBackgroundColor(Integer.MIN_VALUE));
        chat.extractRenderState(g, GuiGame.font, hud.getGuiTicks(), mx, my, ChatComponent.DisplayMode.FOREGROUND, false);
        @SuppressWarnings("unchecked")
        var renderables = (List<Renderable>) Stub.get(screen, Screen.class, "renderables");
        for (var r : renderables) r.extractRenderState(g, mx, my, 1.0F);
        ((CommandSuggestions) Stub.get(screen, ChatScreen.class, "commandSuggestions")).extractRenderState(g, mx, my);
        g.extractDeferredElements(mx, my, 1.0F);
        result.addProperty("value", input.getValue());
        result.addProperty("cursor", input.getCursorPosition());
        result.addProperty("highlight", (int) Stub.get(input, EditBox.class, "highlightPos"));
        result.addProperty("display", (int) Stub.get(input, EditBox.class, "displayPos"));
        result.add("draws", g.draws);
        return result;
    }

    public static void main(String[] args) throws Exception {
        GuiGame.init();
        var gui = Stub.alloc(StubGui.class);
        var mc = client(gui);
        var gson = new Gson();
        var out = new StringBuilder();
        for (String line : Files.readAllLines(Path.of(args[1]))) {
            if (line.isBlank()) continue;
            var c = JsonParser.parseString(line).getAsJsonObject();
            JsonObject result;
            try {
                result = run(c, mc, gui);
            } catch (Throwable error) {
                var cause = error instanceof InvocationTargetException && error.getCause() != null ? error.getCause() : error;
                var sw = new java.io.StringWriter();
                cause.printStackTrace(new java.io.PrintWriter(sw));
                result = new JsonObject();
                result.addProperty("error", sw.toString().lines().limit(10).reduce("", (a, b) -> a + b + " | "));
            }
            var tagged = new JsonObject();
            tagged.add("case", c);
            result.entrySet().forEach(e -> tagged.add(e.getKey(), e.getValue()));
            out.append(gson.toJson(tagged)).append('\n');
        }
        Files.writeString(Path.of(args[0], "vanilla.jsonl"), out.toString(), StandardCharsets.UTF_8);
    }
}
