package acacia.parity.fluids;

import acacia.parity.Game;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.level.block.state.BlockState;

/** The corpus: pools, levels, falls, spills, lava, waterlogged blocks, neighbours, biomes, random mixes. */
final class Cases {
    private Cases() {}

    /** Liquids sit at y 66 on the floor at 65; the centre cell is (7, 66, 7). */
    static final int Y = 66;
    static final int C = 7;

    static final String[] NEIGHBOURS = {"stone", "glass", "white_stained_glass", "tinted_glass", "oak_leaves", "azalea_leaves",
        "ice", "packed_ice", "frosted_ice", "slime_block", "honey_block", "copper_grate", "barrier", "smooth_stone_slab[type=bottom]",
        "smooth_stone_slab[type=top]", "oak_stairs[facing=east,half=bottom]", "oak_stairs[facing=west,half=bottom]",
        "oak_stairs[facing=north,half=top]", "oak_stairs[facing=south,half=bottom,shape=outer_left]", "oak_door[facing=east,half=lower]",
        "oak_door[facing=north,half=lower,open=true]", "oak_door[facing=west,half=upper]", "oak_trapdoor[half=top]",
        "oak_trapdoor[facing=west,open=true]", "oak_fence", "cobblestone_wall", "glass_pane", "iron_bars", "white_carpet",
        "snow[layers=1]", "snow[layers=7]", "snow[layers=8]", "soul_sand", "mud", "farmland", "dirt_path", "chest", "oak_sign",
        "oak_wall_sign[facing=east]", "black_banner", "stone_pressure_plate", "cobweb", "scaffolding", "bamboo", "sea_pickle",
        "lantern", "torch", "ladder[facing=west]", "cactus", "sugar_cane", "sand", "hopper", "enchanting_table", "daylight_detector",
        "lectern", "bell", "anvil", "cauldron", "composter", "mangrove_roots", "moss_carpet", "pointed_dripstone", "amethyst_cluster",
        "big_dripleaf", "beacon", "conduit", "end_portal_frame", "magma_block", "shulker_box", "skeleton_skull", "flower_pot",
        "candle", "decorated_pot", "heavy_core", "oak_hanging_sign", "powder_snow", "light", "structure_void", "spawner",
        "oak_slab[type=double]", "dirt", "grass_block", "glowstone", "sea_lantern", "redstone_block", "piston[facing=up]",
        "piston_head[facing=up]", "end_rod", "iron_chain", "lightning_rod", "brewing_stand", "grindstone", "stonecutter", "campfire",
        "dried_kelp_block", "white_bed[part=foot]", "oak_pressure_plate", "rail", "lever", "repeater",
        "nether_portal", "fire", "short_grass", "vine[east=true]", "glow_lichen[west=true]", "sculk_vein[down=true]", "resin_clump[down=true]"};

    static final String[] WATERLOGGED = {"smooth_stone_slab[type=bottom,waterlogged=true]", "smooth_stone_slab[type=top,waterlogged=true]",
        "oak_stairs[facing=east,half=bottom,waterlogged=true]", "oak_stairs[facing=north,half=top,shape=inner_left,waterlogged=true]",
        "oak_fence[waterlogged=true]", "glass_pane[waterlogged=true]", "iron_bars[waterlogged=true]", "chest[waterlogged=true]",
        "lantern[waterlogged=true]", "oak_trapdoor[half=top,waterlogged=true]", "oak_trapdoor[facing=south,open=true,waterlogged=true]",
        "oak_sign[waterlogged=true]", "sea_pickle[waterlogged=true]", "seagrass", "tall_seagrass[half=lower]", "kelp",
        "bubble_column[drag=true]", "bubble_column[drag=false]", "oak_leaves[waterlogged=true]", "mangrove_roots[waterlogged=true]",
        "copper_grate[waterlogged=true]", "ladder[facing=west,waterlogged=true]", "scaffolding[waterlogged=true]", "conduit",
        "pointed_dripstone[waterlogged=true]", "amethyst_cluster[waterlogged=true]", "heavy_core[waterlogged=true]",
        "decorated_pot[waterlogged=true]", "light[waterlogged=true]", "cobblestone_wall[waterlogged=true]", "iron_chain[waterlogged=true]"};

    static final String[] BIOMES = {"swamp", "mangrove_swamp", "warm_ocean", "frozen_ocean", "cherry_grove", "nether_wastes"};

    static BlockState s(String text) {
        try {
            return BlockStateParser.parseForBlock(BuiltInRegistries.BLOCK, "minecraft:" + text, false).blockState();
        } catch (Exception e) {
            throw new IllegalArgumentException(text, e);
        }
    }

    static BlockState fluid(String fluid, int level) {
        return s(fluid + "[level=" + level + "]");
    }

    static Scene scene(String name, String biome) {
        var key = ResourceKey.create(Registries.BIOME, Identifier.withDefaultNamespace(biome));
        var value = Game.world().lookupOrThrow(Registries.BIOME).getOrThrow(key).value();
        return new Scene(name, biome, value, biome.equals("nether_wastes")).floor();
    }

    static Scene scene(String name) {
        return scene(name, "plains");
    }

    static String id(String text) {
        return text.replaceAll("[^a-z0-9]+", "_").replaceAll("_$", "");
    }

    static List<Scene> all() {
        var out = new ArrayList<Scene>();
        for (var f : new String[]{"water", "lava"}) {
            out.add(pool(scene("pool_" + f), f, 2, 1));
            out.add(pool(scene("pool_deep_" + f), f, 2, 2));
            for (int level = 0; level < 16; level++) out.add(scene("single_" + f + "_" + level).put(C, Y, C, fluid(f, level)));
            var diamond = scene("diamond_" + f);
            for (int dx = -7; dx <= 7; dx++) for (int dz = -7; dz <= 7; dz++) {
                int d = Math.abs(dx) + Math.abs(dz);
                if (d <= 7 && C + dx >= 1 && C + dx <= 14 && C + dz >= 1 && C + dz <= 14) diamond.put(C + dx, Y, C + dz, fluid(f, d));
            }
            out.add(diamond);
            out.add(fall(scene("fall_" + f), f, false));
            out.add(fall(scene("fall_wall_" + f), f, true));
            out.add(spill(f));
        }
        var levels = scene("fall_levels");
        for (int y = Y; y < Y + 8; y++) levels.put(C, y, C, fluid("water", 8 + y - Y)).put(C + 1, y, C, fluid("water", 15 - y + Y));
        out.add(levels);
        for (var biome : BIOMES) {
            out.add(pool(scene("biome_" + biome + "_water", biome), "water", 1, 1).put(C + 3, Y, C, fluid("water", 3)));
            out.add(pool(scene("biome_" + biome + "_lava", biome), "lava", 1, 1).put(C + 3, Y, C, fluid("lava", 2)));
        }
        Neighbours.add(out);
        Mixes.add(out);
        return out;
    }

    /** Sources `r` around the centre, `depth` deep. */
    static Scene pool(Scene scene, String f, int r, int depth) {
        for (int dx = -r; dx <= r; dx++) for (int dz = -r; dz <= r; dz++) for (int y = Y; y < Y + depth; y++) scene.put(C + dx, y, C + dz, fluid(f, 0));
        return scene;
    }

    /** A source up high, falling onto a spreading puddle; `wall` puts stone beside the fall. */
    static Scene fall(Scene scene, String f, boolean wall) {
        scene.put(C, Y + 4, C, fluid(f, 0));
        for (int y = Y; y < Y + 4; y++) scene.put(C, y, C, fluid(f, 8));
        for (int dx = -2; dx <= 2; dx++) for (int dz = -2; dz <= 2; dz++) {
            int d = Math.abs(dx) + Math.abs(dz);
            if (d > 0) scene.put(C + dx, Y, C + dz, fluid(f, d));
        }
        if (wall) scene.put(C + 1, Y + 2, C, s("stone")).put(C, Y + 3, C - 1, s("glass"));
        return scene;
    }

    /** A pool on a ledge running over its edge into a fall. */
    static Scene spill(String f) {
        var scene = scene("spill_" + f);
        for (int x = C + 1; x <= 14; x++) for (int z = 1; z <= 14; z++) scene.blocks.remove(net.minecraft.core.BlockPos.asLong(x, Y - 1, z));
        for (int x = C - 3; x <= C; x++) for (int z = C - 1; z <= C + 1; z++) scene.put(x, Y, z, fluid(f, 0));
        for (int z = C - 1; z <= C + 1; z++) scene.put(C + 1, Y, z, fluid(f, 1)).put(C + 1, Y - 1, z, fluid(f, 8)).put(C + 1, Y - 2, z, fluid(f, 8));
        return scene;
    }

    /** Random mixes of liquids and blocks. */
    static final class Mixes {
        private Mixes() {}

        static final String[] PALETTE = {"air", "air", "air", "air", "stone", "glass", "oak_leaves", "smooth_stone_slab[type=bottom]",
            "smooth_stone_slab[type=top,waterlogged=true]", "oak_stairs[facing=south,half=bottom,waterlogged=true]", "ice",
            "oak_fence[waterlogged=true]", "soul_sand", "oak_sign", "seagrass", "black_banner"};

        static void add(List<Scene> out) {
            var random = new java.util.Random(263);
            for (int i = 0; i < 80; i++) {
                var scene = scene("random_" + i, i % 10 == 9 ? BIOMES[i / 10 % BIOMES.length] : "plains");
                for (int x = 4; x <= 10; x++) for (int z = 4; z <= 10; z++) for (int y = Y; y < Y + 4; y++) {
                    int roll = random.nextInt(10);
                    if (roll < 4) scene.put(x, y, z, fluid(random.nextInt(8) == 0 ? "lava" : "water", random.nextInt(16)));
                    else scene.put(x, y, z, s(PALETTE[random.nextInt(PALETTE.length)]));
                }
                out.add(scene);
            }
        }
    }
}
