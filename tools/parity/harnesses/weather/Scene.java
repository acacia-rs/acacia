package acacia.parity.weather;

import acacia.parity.Out;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Random;
import net.minecraft.core.Holder;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;

/**
 * The columns around a camera: ground height (MOTION_BLOCKING), biome, whether the chunk is loaded,
 * light, and the block the ground ends in. Outside the square: unloaded.
 */
final class Scene {
    final int x0, z0, side;
    final int[] surface, block, sky;
    final String[] biome;
    final boolean[] loaded;
    final BlockState[] top;
    private final Map<String, Holder<Biome>> biomes;

    /** Patches of biomes and unloaded chunks, ground near `groundY`, `palette` for the ground's top block. */
    Scene(Random r, int cx, int cz, int radius, int groundY, Map<String, Holder<Biome>> biomes, List<BlockState> palette) {
        this.biomes = biomes;
        int margin = 2;
        x0 = cx - radius - margin;
        z0 = cz - radius - margin;
        side = 2 * (radius + margin) + 1;
        int n = side * side;
        surface = new int[n];
        block = new int[n];
        sky = new int[n];
        biome = new String[n];
        loaded = new boolean[n];
        top = new BlockState[n];
        var names = new ArrayList<>(biomes.keySet());
        int tiles = side / 4 + 1;
        var tileBiome = new String[tiles * tiles];
        var tileLoaded = new boolean[tiles * tiles];
        var tileGround = new int[tiles * tiles];
        for (int t = 0; t < tileBiome.length; t++) {
            tileBiome[t] = names.get(r.nextInt(names.size()));
            tileLoaded[t] = r.nextInt(12) != 0;
            tileGround[t] = groundY + r.nextInt(9) - 4;
        }
        for (int i = 0; i < n; i++) {
            int tile = (i / side / 4) * tiles + (i % side) / 4;
            surface[i] = r.nextInt(25) == 0 ? groundY + 30 + r.nextInt(30) : tileGround[tile] + r.nextInt(3) - 1;
            biome[i] = tileBiome[tile];
            loaded[i] = tileLoaded[tile];
            block[i] = r.nextInt(16);
            sky[i] = r.nextInt(16);
            top[i] = palette.isEmpty() ? Blocks.STONE.defaultBlockState() : palette.get(r.nextInt(palette.size()));
        }
    }

    private int index(int x, int z) {
        int i = x - x0, k = z - z0;
        return i < 0 || k < 0 || i >= side || k >= side ? -1 : k * side + i;
    }

    boolean loaded(int x, int z) {
        int i = index(x, z);
        return i >= 0 && loaded[i];
    }

    int surface(int x, int z) {
        return surface[index(x, z)];
    }

    Holder<Biome> biome(int x, int z) {
        return biomes.get(biome[index(x, z)]);
    }

    int light(int x, int z, boolean skyLayer) {
        int i = index(x, z);
        return i < 0 ? 0 : skyLayer ? sky[i] : block[i];
    }

    BlockState top(int x, int z) {
        return top[index(x, z)];
    }

    /** What the probe reads (the top blocks are written by whoever needs them). */
    Map<String, Object> json() {
        var light = new ArrayList<int[]>();
        for (int i = 0; i < surface.length; i++) light.add(new int[] {block[i], sky[i]});
        return Out.map("origin", new int[] {x0, z0}, "side", side, "surface", surface, "biome", biome, "loaded", loaded, "light", light);
    }
}
