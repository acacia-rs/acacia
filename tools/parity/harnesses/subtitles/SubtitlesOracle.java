package acacia.parity.gui;

import com.google.gson.Gson;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.blaze3d.audio.ListenerTransform;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.gui.components.SubtitleOverlay;
import net.minecraft.client.resources.sounds.SimpleSoundInstance;
import net.minecraft.client.resources.sounds.SoundInstance;
import net.minecraft.client.sounds.SoundEventListener;
import net.minecraft.client.sounds.SoundManager;
import net.minecraft.client.sounds.WeighedSoundEvents;
import net.minecraft.resources.Identifier;
import net.minecraft.sounds.SoundSource;
import net.minecraft.util.RandomSource;
import net.minecraft.util.Util;
import net.minecraft.world.phys.Vec3;

/**
 * The `subtitles` harness's oracle: vanilla's SubtitleOverlay on a stand-in client. Each case
 * (cases.py) is a script of clock changes (Util.getMillis), sounds the engine reports to its
 * listeners and frames; every frame renders the overlay, the last one into GuiCapture. Output
 * lines: {case, draws} or {case, error}. Adapted from Horizon's VanillaSubtitleOracle (MIT,
 * (c) Ludovic Marie).
 */
public final class SubtitlesOracle {
    static long nanos;

    /** The sound manager's listener list and listener transform. */
    public static final class Sounds extends SoundManager {
        List<SoundEventListener> listeners;
        ListenerTransform transform;

        private Sounds() { super(null); }

        @Override public void addListener(SoundEventListener listener) { listeners.add(listener); }
        @Override public void removeListener(SoundEventListener listener) { listeners.remove(listener); }
        @Override public ListenerTransform getListenerTransform() { return transform; }
    }

    static <T> net.minecraft.client.OptionInstance<T> option(T value) throws Exception {
        @SuppressWarnings("unchecked") net.minecraft.client.OptionInstance<T> o = Stub.alloc(net.minecraft.client.OptionInstance.class);
        Stub.put(o, net.minecraft.client.OptionInstance.class, "value", value);
        return o;
    }

    static Vec3 vec(JsonElement e) {
        var a = e.getAsJsonArray();
        return new Vec3(a.get(0).getAsDouble(), a.get(1).getAsDouble(), a.get(2).getAsDouble());
    }

    static GuiCapture replay(JsonObject c) throws Exception {
        var mc = Stub.alloc(Minecraft.class);
        var sounds = Stub.alloc(Sounds.class);
        sounds.listeners = new ArrayList<>();
        sounds.transform = ListenerTransform.INITIAL;
        Stub.put(mc, Minecraft.class, "soundManager", sounds);
        Stub.put(mc, Minecraft.class, "font", GuiGame.font);
        var options = Stub.alloc(Options.class);
        Stub.put(mc, Minecraft.class, "options", options);
        var overlay = new SubtitleOverlay(mc);
        GuiCapture last = null;
        for (var e : c.getAsJsonArray("ops")) {
            var op = e.getAsJsonObject();
            switch (op.get("op").getAsString()) {
                case "time" -> nanos = op.get("ms").getAsLong() * 1_000_000L + op.get("sub").getAsLong();
                case "play" -> {
                    var at = vec(op.get("at"));
                    var key = op.get("key");
                    var event = new WeighedSoundEvents(Identifier.withDefaultNamespace("test"), key.isJsonNull() ? null : key.getAsString());
                    SoundInstance sound = new SimpleSoundInstance(Identifier.withDefaultNamespace("test"), SoundSource.MASTER, 1.0F, 1.0F,
                        RandomSource.create(0L), op.get("looping").getAsBoolean(), 0, SoundInstance.Attenuation.LINEAR, at.x, at.y, at.z, false);
                    float range = op.get("range").isJsonNull() ? Float.POSITIVE_INFINITY : op.get("range").getAsFloat();
                    for (var listener : List.copyOf(sounds.listeners)) listener.onPlaySound(sound, event, range);
                }
                case "frame" -> {
                    sounds.transform = new ListenerTransform(vec(op.get("pos")), vec(op.get("forward")), vec(op.get("up")));
                    Stub.put(options, Options.class, "showSubtitles", option(op.get("show").getAsBoolean()));
                    Stub.put(options, Options.class, "notificationDisplayTime", option(op.get("mult").getAsDouble()));
                    Stub.put(options, Options.class, "textBackgroundOpacity", option(op.get("opacity").getAsDouble()));
                    Stub.put(options, Options.class, "backgroundForChatOnly", option(op.get("chatOnly").getAsBoolean()));
                    last = GuiCapture.create(mc, op.get("w").getAsInt(), op.get("h").getAsInt());
                    overlay.extractRenderState(last);
                }
                default -> throw new IllegalArgumentException(op.toString());
            }
        }
        return last;
    }

    public static void main(String[] args) throws Exception {
        GuiGame.init();
        Util.setTimeSource(() -> nanos);
        var gson = new Gson();
        var out = new StringBuilder();
        for (String line : Files.readAllLines(Path.of(args[1]), StandardCharsets.UTF_8)) {
            if (line.isBlank()) continue;
            var c = JsonParser.parseString(line).getAsJsonObject();
            var result = new JsonObject();
            result.add("case", c);
            try {
                result.add("draws", replay(c).draws);
            } catch (Throwable error) {
                result.addProperty("error", error.toString());
            }
            out.append(gson.toJson(result)).append('\n');
        }
        Files.writeString(Path.of(args[0], "vanilla.jsonl"), out.toString(), StandardCharsets.UTF_8);
    }
}
