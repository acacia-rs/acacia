package acacia.parity.gui;

import com.google.gson.Gson;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.authlib.GameProfile;
import com.mojang.authlib.properties.PropertyMap;
import com.mojang.serialization.JsonOps;
import java.lang.reflect.InvocationTargetException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.function.Supplier;
import net.minecraft.client.Minecraft;
import net.minecraft.client.OptionInstance;
import net.minecraft.client.Options;
import net.minecraft.client.gui.Hud;
import net.minecraft.client.gui.components.PlayerTabOverlay;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.player.RemotePlayer;
import net.minecraft.client.resources.DefaultPlayerSkin;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.ComponentSerialization;
import net.minecraft.network.chat.Style;
import net.minecraft.network.chat.numbers.BlankFormat;
import net.minecraft.network.chat.numbers.FixedFormat;
import net.minecraft.network.chat.numbers.NumberFormat;
import net.minecraft.network.chat.numbers.StyledFormat;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.entity.player.PlayerSkin;
import net.minecraft.world.level.GameType;
import net.minecraft.world.scores.Objective;
import net.minecraft.world.scores.ScoreHolder;
import net.minecraft.world.scores.Scoreboard;
import net.minecraft.world.scores.TeamColor;
import net.minecraft.world.scores.criteria.ObjectiveCriteria;

/**
 * The `players` harness's oracle: vanilla's PlayerTabOverlay.extractRenderState, frame after frame,
 * on a stand-in client (connection listing the case's PlayerInfos with default skins, online mode,
 * level holding some of them, a real Scoreboard with teams, `list` objective and scores, options and
 * Hud ticks); the last frame is recorded by GuiCapture. Visibility changes clear the heart states as
 * setVisible does, without the narration. Output lines: {case, draws} or {case, error}. Adapted from
 * Horizon's VanillaTabListOracle (MIT, (c) Ludovic Marie).
 */
public final class PlayersOracle {
    static Component comp(JsonElement e) {
        return ComponentSerialization.CODEC.parse(JsonOps.INSTANCE, e).getOrThrow();
    }

    static NumberFormat format(JsonElement e) {
        if (e == null || e.isJsonNull()) return null;
        var o = e.getAsJsonObject();
        return switch (o.get("type").getAsString()) {
            case "blank" -> BlankFormat.INSTANCE;
            case "styled" -> new StyledFormat(Style.Serializer.CODEC.parse(JsonOps.INSTANCE, o.get("style")).getOrThrow());
            case "fixed" -> new FixedFormat(comp(o.get("value")));
            default -> throw new IllegalArgumentException(o.toString());
        };
    }

    static <T> OptionInstance<T> option(T value) throws Exception {
        @SuppressWarnings("unchecked") OptionInstance<T> o = Stub.alloc(OptionInstance.class);
        Stub.put(o, OptionInstance.class, "value", value);
        return o;
    }

    static GuiCapture replay(JsonObject c) throws Exception {
        var board = new Scoreboard();
        for (var t : c.getAsJsonArray("teams")) {
            var o = t.getAsJsonObject();
            var team = board.addPlayerTeam(o.get("name").getAsString());
            if (!o.get("color").isJsonNull()) team.setColor(Optional.of(TeamColor.byName(o.get("color").getAsString())));
            team.setPlayerPrefix(comp(o.get("prefix")));
            team.setPlayerSuffix(comp(o.get("suffix")));
            for (var m : o.getAsJsonArray("members")) board.addPlayerToTeam(m.getAsString(), team);
        }
        Objective objective = null;
        if (!c.get("objective").isJsonNull()) {
            var o = c.getAsJsonObject("objective");
            var type = o.get("renderType").getAsString().equals("hearts") ? ObjectiveCriteria.RenderType.HEARTS : ObjectiveCriteria.RenderType.INTEGER;
            objective = board.addObjective("list", ObjectiveCriteria.DUMMY, Component.literal("x"), type, false, format(o.get("numberFormat")));
        }
        var mc = Stub.alloc(Minecraft.class);
        Stub.putStatic(Minecraft.class, "instance", mc);
        var level = Stub.alloc(ClientLevel.class);
        var levelPlayers = new ArrayList<net.minecraft.client.player.AbstractClientPlayer>();
        Stub.put(level, ClientLevel.class, "players", levelPlayers);
        Stub.put(mc, Minecraft.class, "level", level);
        Stub.put(mc, Minecraft.class, "font", GuiGame.font);
        var options = Stub.alloc(Options.class);
        Stub.put(options, Options.class, "backgroundForChatOnly", option(c.get("chatOnly").getAsBoolean()));
        Stub.put(options, Options.class, "textBackgroundOpacity", option(c.get("opacity").getAsDouble()));
        Stub.put(mc, Minecraft.class, "options", options);
        var connection = Stub.alloc(ClientPacketListener.class);
        Stub.put(connection, ClientPacketListener.class, "scoreboard", board);
        Stub.put(level, ClientLevel.class, "connection", connection);
        Stub.put(connection, ClientPacketListener.class, "onlineMode", c.get("onlineMode").getAsBoolean());
        var listed = new LinkedHashSet<PlayerInfo>();
        Stub.put(connection, ClientPacketListener.class, "listedPlayers", listed);
        var player = Stub.alloc(LocalPlayer.class);
        Stub.put(player, LocalPlayer.class, "connection", connection);
        Stub.put(mc, Minecraft.class, "player", player);
        for (var p : c.getAsJsonArray("players")) {
            var o = p.getAsJsonObject();
            var uuid = UUID.fromString(o.get("uuid").getAsString());
            var profile = new GameProfile(uuid, o.get("name").getAsString(), new PropertyMap(com.google.common.collect.LinkedHashMultimap.create()));
            var info = Stub.alloc(PlayerInfo.class);
            Stub.put(info, PlayerInfo.class, "profile", profile);
            PlayerSkin skin = DefaultPlayerSkin.get(profile);
            Stub.put(info, PlayerInfo.class, "skinLookup", (Supplier<PlayerSkin>) () -> skin);
            Stub.put(info, PlayerInfo.class, "gameMode", GameType.byId(o.get("gameMode").getAsInt()));
            Stub.put(info, PlayerInfo.class, "latency", o.get("latency").getAsInt());
            if (!o.get("displayName").isJsonNull()) info.setTabListDisplayName(comp(o.get("displayName")));
            info.setTabListOrder(o.get("listOrder").getAsInt());
            info.setShowHat(o.get("showHat").getAsBoolean());
            listed.add(info);
            if (o.get("inLevel").getAsBoolean()) {
                var entity = Stub.alloc(RemotePlayer.class);
                Stub.put(entity, Player.class, "gameProfile", profile);
                Stub.put(entity, net.minecraft.world.entity.Entity.class, "uuid", uuid);
                levelPlayers.add(entity);
            }
        }
        var hud = Stub.alloc(Hud.class);
        Stub.put(hud, Hud.class, "minecraft", mc);
        var overlay = new PlayerTabOverlay(mc, hud);
        if (!c.get("header").isJsonNull()) overlay.setHeader(comp(c.get("header")));
        if (!c.get("footer").isJsonNull()) overlay.setFooter(comp(c.get("footer")));
        @SuppressWarnings("unchecked") var health = (Map<UUID, ?>) Stub.get(overlay, PlayerTabOverlay.class, "healthStates");
        int width = c.get("width").getAsInt();
        GuiCapture capture = null;
        for (var f : c.getAsJsonArray("frames")) {
            var o = f.getAsJsonObject();
            if (objective != null) {
                for (var s : o.getAsJsonObject("scores").entrySet()) {
                    var v = s.getValue().getAsJsonObject();
                    var access = board.getOrCreatePlayerScore(ScoreHolder.forNameOnly(s.getKey()), objective);
                    access.set(v.get("value").getAsInt());
                    access.numberFormatOverride(format(v.get("format")));
                }
                for (var r : o.getAsJsonArray("reset")) board.resetSinglePlayerScore(ScoreHolder.forNameOnly(r.getAsString()), objective);
            }
            Stub.put(hud, Hud.class, "tickCount", o.get("tick").getAsInt());
            boolean visible = o.get("visible").getAsBoolean();
            if ((boolean) Stub.get(overlay, PlayerTabOverlay.class, "visible") != visible) {
                health.clear();
                Stub.put(overlay, PlayerTabOverlay.class, "visible", visible);
            }
            capture = GuiCapture.create(mc, width, 240);
            if (visible) overlay.extractRenderState(capture, width, board, objective);
        }
        return capture;
    }

    public static void main(String[] args) throws Exception {
        GuiGame.init();
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
                var cause = error instanceof InvocationTargetException && error.getCause() != null ? error.getCause() : error;
                var sw = new java.io.StringWriter();
                cause.printStackTrace(new java.io.PrintWriter(sw));
                result.addProperty("error", sw.toString().lines().limit(8).reduce("", (a, b) -> a + b + " | "));
            }
            out.append(gson.toJson(result)).append('\n');
        }
        Files.writeString(Path.of(args[0], "vanilla.jsonl"), out.toString(), StandardCharsets.UTF_8);
    }
}
