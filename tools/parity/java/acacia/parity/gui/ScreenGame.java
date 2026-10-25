package acacia.parity.gui;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.mojang.blaze3d.platform.TextInputManager;
import com.mojang.serialization.JsonOps;
import java.lang.reflect.Proxy;
import java.nio.file.Files;
import net.minecraft.client.GameNarrator;
import net.minecraft.client.InputType;
import net.minecraft.client.Minecraft;
import net.minecraft.client.OptionInstance;
import net.minecraft.client.Options;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.ComponentSerialization;

/**
 * The stand-in client the screen and widget oracles draw on: `Minecraft.getInstance()` with the
 * default font's metrics, a real `Options` (defaults, nothing read), a window of the case's GUI
 * size, no narrator, no text input and (when `inWorld`) a level, so screens pick their in-world
 * textures. Fields are written without running constructors (Stub).
 */
public final class ScreenGame {
    private ScreenGame() {}

    private static java.io.File optionsDir;

    /** A TextInputManager that talks to no window. */
    public static final class NoTextInput extends TextInputManager {
        private NoTextInput() { super(null); }
        @Override public void setTextInputArea(int x0, int y0, int x1, int y1) { }
        @Override public void onTextInputFocusChange(Object owner, boolean focused) { }
        @Override public void startTextInput(Object owner) { }
        @Override public void stopTextInput() { }
    }

    public static Minecraft create(int width, int height, boolean inWorld) throws Exception {
        var mc = Stub.alloc(Minecraft.class);
        Stub.putStatic(Minecraft.class, "instance", mc);
        Stub.put(mc, Minecraft.class, "font", GuiGame.font);
        var window = Stub.alloc(com.mojang.blaze3d.platform.Window.class);
        Stub.put(window, com.mojang.blaze3d.platform.Window.class, "guiScaledWidth", width);
        Stub.put(window, com.mojang.blaze3d.platform.Window.class, "guiScaledHeight", height);
        Stub.put(mc, Minecraft.class, "window", window);
        var gui = Stub.alloc(net.minecraft.client.gui.Gui.class);
        Stub.put(gui, net.minecraft.client.gui.Gui.class, "hud", Stub.alloc(net.minecraft.client.gui.Hud.class));
        Stub.put(mc, Minecraft.class, "gui", gui);
        Stub.put(mc, Minecraft.class, "textInputManager", Stub.alloc(NoTextInput.class));
        Stub.put(mc, Minecraft.class, "lastInputType", InputType.MOUSE);
        var narrator = Stub.alloc(GameNarrator.class);
        Object quiet = Proxy.newProxyInstance(ScreenGame.class.getClassLoader(), new Class<?>[] {com.mojang.text2speech.Narrator.class},
            (proxy, method, args) -> method.getReturnType() == boolean.class ? false : null);
        Stub.put(narrator, GameNarrator.class, "narrator", quiet);
        Stub.put(mc, Minecraft.class, "narrator", narrator);
        if (optionsDir == null) optionsDir = Files.createTempDirectory("options").toFile();
        Stub.put(mc, Minecraft.class, "options", new Options(mc, optionsDir));
        if (inWorld) Stub.put(mc, Minecraft.class, "level", Stub.alloc(net.minecraft.client.multiplayer.ClientLevel.class));
        return mc;
    }

    /** A component from the case's JSON (a string is literal text). */
    public static Component component(JsonElement e) {
        if (e == null || e.isJsonNull()) return Component.empty();
        if (e.isJsonPrimitive()) return Component.literal(e.getAsString());
        return ComponentSerialization.CODEC.parse(JsonOps.INSTANCE, e).getOrThrow();
    }

    public static <T> void set(OptionInstance<T> option, T value) throws Exception {
        Stub.put(option, OptionInstance.class, "value", value);
    }

    /** What one case records. */
    public interface Kind {
        com.google.gson.JsonArray draws(JsonObject c, JsonObject result) throws Exception;
    }

    /**
     * The client's en_us as the language: `Language.loadDefault` reads whichever en_us.json the class
     * path finds first, the server jar's, which lacks the client's menu keys.
     */
    static void injectClientLanguage() throws Exception {
        var jar = new java.io.File(Minecraft.class.getProtectionDomain().getCodeSource().getLocation().toURI());
        var storage = new java.util.HashMap<String, String>();
        try (var zip = new java.util.zip.ZipFile(jar); var in = zip.getInputStream(zip.getEntry("assets/minecraft/lang/en_us.json"))) {
            net.minecraft.locale.Language.loadFromJson(in, storage::put);
        }
        var fallback = net.minecraft.locale.Language.getInstance();
        net.minecraft.locale.Language.inject(new net.minecraft.locale.Language() {
            @Override public String getOrDefault(String id, String d) { return storage.getOrDefault(id, d); }
            @Override public boolean has(String id) { return storage.containsKey(id); }
            @Override public boolean isDefaultRightToLeft() { return false; }
            @Override public net.minecraft.util.FormattedCharSequence getVisualOrder(net.minecraft.network.chat.FormattedText t) { return fallback.getVisualOrder(t); }
        });
    }

    /** An oracle's main: `args[0]` the out dir (vanilla.jsonl), `args[1]` the cases, one per line. */
    public static void run(String[] args, Kind kind) throws Exception {
        GuiGame.init();
        injectClientLanguage();
        var gson = new com.google.gson.Gson();
        var out = new StringBuilder();
        for (String line : Files.readAllLines(java.nio.file.Path.of(args[1]))) {
            if (line.isBlank()) continue;
            var c = com.google.gson.JsonParser.parseString(line).getAsJsonObject();
            var result = new JsonObject();
            result.add("case", c);
            try {
                result.add("draws", kind.draws(c, result));
            } catch (Throwable error) {
                var cause = error instanceof java.lang.reflect.InvocationTargetException && error.getCause() != null ? error.getCause() : error;
                var sw = new java.io.StringWriter();
                cause.printStackTrace(new java.io.PrintWriter(sw));
                result.addProperty("error", sw.toString().lines().limit(10).reduce("", (a, b) -> a + b + " | "));
            }
            out.append(gson.toJson(result)).append('\n');
        }
        Files.writeString(java.nio.file.Path.of(args[0], "vanilla.jsonl"), out.toString(), java.nio.charset.StandardCharsets.UTF_8);
    }

    public static int i(JsonObject c, String k, int d) { return c.has(k) && !c.get(k).isJsonNull() ? c.get(k).getAsInt() : d; }
    public static float f(JsonObject c, String k, float d) { return c.has(k) && !c.get(k).isJsonNull() ? c.get(k).getAsFloat() : d; }
    public static boolean b(JsonObject c, String k, boolean d) { return c.has(k) && !c.get(k).isJsonNull() ? c.get(k).getAsBoolean() : d; }
    public static String s(JsonObject c, String k, String d) { return c.has(k) && !c.get(k).isJsonNull() ? c.get(k).getAsString() : d; }
}
