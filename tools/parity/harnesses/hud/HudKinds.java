package acacia.parity.gui;

import static acacia.parity.gui.HudActor.b;
import static acacia.parity.gui.HudActor.f;
import static acacia.parity.gui.HudActor.i;
import static acacia.parity.gui.HudOracle.option;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.UUID;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.Hud;
import net.minecraft.client.gui.contextualbar.ExperienceBar;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.ComponentSerialization;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.GameType;

/** The `hud` oracle's case kinds (cases.py); each draws into the case's capture. */
final class HudKinds {
    private HudKinds() {}

    static Component comp(JsonElement e) {
        return ComponentSerialization.CODEC.parse(com.mojang.serialization.JsonOps.INSTANCE, e).getOrThrow();
    }

    static Method hud(String name, Class<?>... params) throws Exception {
        return Stub.method(Hud.class, name, params);
    }

    static net.minecraft.network.chat.numbers.NumberFormat numberFormat(JsonElement e) {
        if (e == null || e.isJsonNull()) return null;
        var o = e.getAsJsonObject();
        return switch (o.get("type").getAsString()) {
            case "blank" -> net.minecraft.network.chat.numbers.BlankFormat.INSTANCE;
            case "fixed" -> new net.minecraft.network.chat.numbers.FixedFormat(comp(o.get("value")));
            case "styled" -> new net.minecraft.network.chat.numbers.StyledFormat(
                net.minecraft.network.chat.Style.Serializer.MAP_CODEC.codec().parse(com.mojang.serialization.JsonOps.INSTANCE, o.get("style")).getOrThrow());
            default -> throw new IllegalArgumentException(o.toString());
        };
    }

    static void run(String kind, HudOracle.Ctx x, JsonObject c) throws Exception {
        var G = GuiGraphicsExtractor.class;
        var D = DeltaTracker.class;
        // Hud.tick's vignette brightness reads the camera entity's level: tick without one.
        if (Set.of("title", "selected").contains(kind)) Stub.put(x.camera, net.minecraft.client.Camera.class, "entity", null);
        switch (kind) {
            case "decorations" -> {
                if (b("crosshair")) {
                    Stub.put(x.options, Options.class, "cameraType", net.minecraft.client.CameraType.FIRST_PERSON);
                    var debug = Stub.alloc(net.minecraft.client.gui.components.debug.DebugScreenEntryList.class);
                    Stub.put(debug, net.minecraft.client.gui.components.debug.DebugScreenEntryList.class, "currentlyEnabled", new ArrayList<Identifier>());
                    Stub.put(x.mc, Minecraft.class, "debugEntries", debug);
                    boolean target = c.has("target") && !c.get("target").isJsonNull();
                    Stub.put(x.mc, Minecraft.class, "crosshairPickEntity", target ? Stub.alloc(HudActor.Target.class) : null);
                    hud("extractCrosshair", G, D).invoke(x.hud, x.capture, x.delta);
                }
                if (x.mode != GameType.SPECTATOR) hud("extractItemHotbar", G, D).invoke(x.hud, x.capture, x.delta);
                if (x.mode.isSurvival()) hud("extractPlayerHealth", G).invoke(x.hud, x.capture);
                hud("extractVehicleHealth", G).invoke(x.hud, x.capture);
                if (b("experienceBar")) new ExperienceBar(x.mc).extractBackground(x.capture, x.delta);
                if (x.mode.isSurvival() && x.actor.experienceLevel > 0) {
                    Stub.method(Class.forName("net.minecraft.client.gui.contextualbar.ContextualBar"), "extractExperienceLevel", G, Font.class, int.class)
                        .invoke(null, x.capture, GuiGame.font, x.actor.experienceLevel);
                }
            }
            case "effects" -> {
                for (var e : c.getAsJsonArray("active")) {
                    var o = e.getAsJsonObject();
                    var effect = BuiltInRegistries.MOB_EFFECT.get(Identifier.parse(o.get("id").getAsString())).orElseThrow();
                    x.actor.active.add(new net.minecraft.world.effect.MobEffectInstance(effect, o.get("duration").getAsInt(), 0,
                        o.get("ambient").getAsBoolean(), true, o.get("icon").getAsBoolean()));
                }
                hud("extractEffects", G, D).invoke(x.hud, x.capture, x.delta);
            }
            case "title" -> {
                var tick = hud("tick");
                for (var e : c.getAsJsonArray("ops")) {
                    var op = e.getAsJsonArray();
                    switch (op.get(0).getAsString()) {
                        case "title" -> x.hud.setTitle(comp(op.get(1)));
                        case "subtitle" -> x.hud.setSubtitle(comp(op.get(1)));
                        case "times" -> x.hud.setTimes(op.get(1).getAsInt(), op.get(2).getAsInt(), op.get(3).getAsInt());
                        case "clear" -> x.hud.clearTitles();
                        case "reset" -> x.hud.resetTitleTimes();
                        case "overlay" -> x.hud.setOverlayMessage(comp(op.get(1)), op.get(2).getAsBoolean());
                        case "tick" -> { for (int n = op.get(1).getAsInt(); n > 0; n--) tick.invoke(x.hud); }
                        default -> throw new IllegalArgumentException(op.toString());
                    }
                }
                hud("extractOverlayMessage", G, D).invoke(x.hud, x.capture, x.delta);
                hud("extractTitle", G, D).invoke(x.hud, x.capture, x.delta);
            }
            case "selected" -> {
                int slot = i("selected", 0);
                var tick = hud("tick");
                for (var e : c.getAsJsonArray("steps")) {
                    ItemStack stack = ItemStack.EMPTY;
                    if (!e.isJsonNull()) {
                        var o = e.getAsJsonObject();
                        stack = new ItemStack(BuiltInRegistries.ITEM.getValue(Identifier.withDefaultNamespace(o.get("item").getAsString())));
                        if (!o.get("custom").isJsonNull()) stack.set(net.minecraft.core.component.DataComponents.CUSTOM_NAME, comp(o.get("custom")));
                    }
                    x.inventory.setItem(slot, stack);
                    tick.invoke(x.hud);
                }
                hud("extractSelectedItemName", G).invoke(x.hud, x.capture);
            }
            case "boss" -> {
                var overlay = new net.minecraft.client.gui.components.BossHealthOverlay(x.mc);
                @SuppressWarnings("unchecked") var events = (Map<UUID, net.minecraft.client.gui.components.LerpingBossEvent>)
                    Stub.get(overlay, net.minecraft.client.gui.components.BossHealthOverlay.class, "events");
                for (var e : c.getAsJsonArray("bars")) {
                    var o = e.getAsJsonObject();
                    var id = UUID.randomUUID();
                    events.put(id, new net.minecraft.client.gui.components.LerpingBossEvent(id, comp(o.get("name")), o.get("progress").getAsFloat(),
                        net.minecraft.world.BossEvent.BossBarColor.values()[o.get("color").getAsInt()],
                        net.minecraft.world.BossEvent.BossBarOverlay.values()[o.get("overlay").getAsInt()], false, false, false));
                }
                overlay.extractRenderState(x.capture);
            }
            case "jump" -> new net.minecraft.client.gui.contextualbar.JumpableVehicleBar(x.mc).extractBackground(x.capture, x.delta);
            case "sidebar" -> sidebar(x, c);
            case "decor" -> decor(x, c);
            case "overlays" -> HudOverlays.run(x, c);
            case "chat" -> HudOverlays.chat(x, c);
            default -> throw new IllegalArgumentException(kind);
        }
    }

    static void sidebar(HudOracle.Ctx x, JsonObject c) throws Exception {
        var board = new net.minecraft.world.scores.Scoreboard();
        var o = c.getAsJsonObject("objective");
        var objective = board.addObjective("sidebar", net.minecraft.world.scores.criteria.ObjectiveCriteria.DUMMY, comp(o.get("display")),
            net.minecraft.world.scores.criteria.ObjectiveCriteria.RenderType.INTEGER, false, numberFormat(o.get("format")));
        for (var e : c.getAsJsonArray("scores")) {
            var sc = e.getAsJsonObject();
            var access = board.getOrCreatePlayerScore(net.minecraft.world.scores.ScoreHolder.forNameOnly(sc.get("owner").getAsString()), objective);
            access.set(sc.get("value").getAsInt());
            if (!sc.get("display").isJsonNull()) access.display(comp(sc.get("display")));
            access.numberFormatOverride(numberFormat(sc.get("format")));
        }
        for (var e : c.getAsJsonArray("teams")) {
            var t = e.getAsJsonObject();
            var team = board.addPlayerTeam(t.get("name").getAsString());
            team.setPlayerPrefix(comp(t.get("prefix")));
            team.setPlayerSuffix(comp(t.get("suffix")));
            team.setColor(t.get("color").isJsonNull() ? Optional.empty() : Optional.of(net.minecraft.world.scores.TeamColor.byName(t.get("color").getAsString())));
            for (var m : t.getAsJsonArray("members")) board.addPlayerToTeam(m.getAsString(), team);
        }
        hud("displayScoreboardSidebar", GuiGraphicsExtractor.class, net.minecraft.world.scores.Objective.class).invoke(x.hud, x.capture, objective);
    }

    static void decor(HudOracle.Ctx x, JsonObject c) throws Exception {
        var stack = new ItemStack(BuiltInRegistries.ITEM.getValue(Identifier.withDefaultNamespace(c.get("item").getAsString())));
        stack.setCount(i("count", 1));
        stack.setDamageValue(i("damage", 0));
        if (b("unbreakable")) stack.set(net.minecraft.core.component.DataComponents.UNBREAKABLE, net.minecraft.util.Unit.INSTANCE);
        var cooldowns = new net.minecraft.world.item.ItemCooldowns();
        Stub.put(x.actor, net.minecraft.world.entity.player.Player.class, "cooldowns", cooldowns);
        if (c.has("cooldown") && !c.get("cooldown").isJsonNull()) {
            var cd = c.getAsJsonObject("cooldown");
            cooldowns.addCooldown(stack, cd.get("duration").getAsInt());
            for (int n = cd.get("elapsed").getAsInt(); n > 0; n--) cooldowns.tick();
        }
        var timer = Stub.alloc(DeltaTracker.Timer.class);
        Stub.put(timer, DeltaTracker.Timer.class, "deltaTickResidual", x.partial);
        Stub.put(x.mc, Minecraft.class, "deltaTracker", timer);
        x.capture.realDecorations = true;
        x.capture.itemDecorations(GuiGame.font, stack, i("x", 0), i("y", 0), c.has("countText") ? c.get("countText").getAsString() : null);
    }
}
