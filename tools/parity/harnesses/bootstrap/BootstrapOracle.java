package acacia.parity.bootstrap;

import acacia.parity.Client;
import acacia.parity.Dump;
import acacia.parity.Gui;
import acacia.parity.Out;
import net.minecraft.ChatFormatting;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Callable;
import net.minecraft.core.BlockPos;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.EntityTypes;
import net.minecraft.world.level.block.Blocks;

/** Exercises each piece of the shared bootstrap; one line per check: {case, ok, detail}. */
public final class BootstrapOracle {
    private static final List<Object[]> CHECKS = new ArrayList<>();

    private static void check(String name, Callable<Object> body) {
        CHECKS.add(new Object[]{name, body});
    }

    public static void main(String[] args) throws Exception {
        Client.minecraft();
        check("level: a set block reads back", () -> {
            var level = Client.overworld();
            Client.setBlock(level, new BlockPos(3, 70, -5), Blocks.OAK_LOG.defaultBlockState());
            var got = level.getBlockState(new BlockPos(3, 70, -5));
            return got == Blocks.OAK_LOG.defaultBlockState() ? null : "read " + got;
        });
        for (EntityType<?> type : List.of(EntityTypes.PIG, EntityTypes.ZOMBIE, EntityTypes.ARMOR_STAND, EntityTypes.ITEM_FRAME, EntityTypes.CREEPER)) {
            check("entity render state: " + EntityType.getKey(type), () -> {
                var level = Client.overworld();
                var entity = Client.spawn(level, type, 0.5, 64, 0.5);
                var state = Client.entityRenderers().extractEntity(entity, 0.0F);
                return state == null ? "no state" : null;
            });
        }
        check("gui: sprite, fill, text and item records", () -> {
            Client.overworld();
            var font = Client.minecraft().font;
            var frame = Gui.record(0, 0, g -> {
                g.blitSprite(RenderPipelines.GUI_TEXTURED, Identifier.withDefaultNamespace("hud/heart/full"), 10, 20, 9, 9);
                g.fill(0, 0, 5, 5, 0x80FF0000);
                g.text(font,Component.literal("Hi ").append(Component.literal("there").withStyle(ChatFormatting.RED)), 4, 30, -1);
                g.item(new ItemStack(Items.DIAMOND_SWORD), 40, 40);
            });
            Out.json(Path.of(args[0]).resolve("gui.json"), frame);
            var blit = String.valueOf(((List<?>) frame.get("elements")).getFirst());
            if (!blit.contains("gui.png#minecraft:hud/heart/full")) return "first element " + blit;
            if (((List<?>) frame.get("text")).size() != 1 || ((List<?>) frame.get("items")).size() != 1) return "text/items " + frame.keySet();
            return null;
        });
        check("gui frame: draws in GuiRenderer order", () -> {
            Client.overworld();
            var font = Client.minecraft().font;
            var frame = Gui.frame(0, 0, g -> {
                g.blitSprite(RenderPipelines.GUI_TEXTURED, Identifier.withDefaultNamespace("widget/button"), 10, 10, 120, 20);
                g.text(font, "Done", 50, 16, -1);
                g.item(new ItemStack(Items.APPLE), 140, 10);
            });
            Out.json(Path.of(args[0]).resolve("gui-frame.json"), frame);
            var draws = (List<?>) frame.get("draws");
            return draws.size() == 3 + 1 + 4 ? null : draws.size() + " draws, want 3 slices, 1 item, 4 glyphs";
        });
        check("view: lightmap and fog at noon in plains", () -> {
            var level = Client.overworld();
            Client.setBiome(level, 0, 0, net.minecraft.world.level.biome.Biomes.PLAINS);
            acacia.parity.View.player(level, 8.5, 64, 8.5, 0.0F, 0.0F);
            var camera = acacia.parity.View.camera(0.0F);
            var light = Dump.of(acacia.parity.View.lightmap(0.0F));
            var fog = Dump.of(acacia.parity.View.fog(camera, 0.0F));
            Out.json(Path.of(args[0]).resolve("view.json"), Out.map("lightmap", light, "fog", fog));
            return light instanceof java.util.Map<?, ?> m && Boolean.TRUE.equals(m.get("needsUpdate")) ? null : "lightmap " + light;
        });
        check("entity render state dump: pig", () -> {
            var level = Client.overworld();
            var pig = Client.spawn(level, EntityTypes.PIG, 0.5, 64, 0.5);
            var tree = Dump.of(Client.entityRenderers().extractEntity(pig, 0.0F));
            Out.json(Path.of(args[0]).resolve("pig.json"), tree);
            return tree instanceof java.util.Map<?, ?> m && m.containsKey("bodyRot") ? null : "fields " + tree;
        });
        try (var out = new Out.Lines(Path.of(args[0]).resolve("checks.jsonl"))) {
            for (var c : CHECKS) {
                String detail;
                try {
                    var r = ((Callable<?>) c[1]).call();
                    detail = r == null ? null : r.toString();
                } catch (Throwable t) {
                    detail = t.toString();
                    t.printStackTrace();
                }
                out.write(Out.map("case", c[0], "ok", detail == null, "detail", detail));
            }
        }
    }
}
