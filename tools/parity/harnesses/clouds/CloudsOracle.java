package acacia.parity.clouds;

import acacia.parity.Game;
import acacia.parity.Out;
import com.mojang.blaze3d.systems.RenderSystem;
import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Random;
import javax.imageio.ImageIO;
import net.minecraft.client.CloudStatus;
import net.minecraft.client.renderer.CloudRenderer;
import net.minecraft.resources.Identifier;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.util.profiling.InactiveProfiler;
import net.minecraft.world.phys.Vec3;

/**
 * CloudRenderer over the vanilla clouds.png and small made-up maps: the cells and their empty
 * neighbours as the texture loader packs them, and per frame (CloudRenderer.prepare) the uniforms it
 * writes (CloudColor, CloudOffset, CellSize) and the faces its mesh builder emits, in order. GPU
 * buffers are stood in by heap-free direct buffers ({@link FakeRing}). Args: out dir, frame cases.
 */
public final class CloudsOracle {
    public static void main(String[] args) throws Exception {
        Game.init();
        RenderSystem.initRenderThread();
        Path out = Path.of(args[0]);
        int cases = Integer.parseInt(args[1]);
        var textures = textures(out.resolve("textures"));
        try (var records = new Out.Lines(out.resolve("textures.jsonl"))) {
            for (var e : textures.entrySet()) records.write(Out.map("case", "texture-" + e.getKey(), "cells", cells(e.getValue())));
        }
        var names = new ArrayList<>(textures.keySet());
        var r = new Random(5);
        try (var in = new Out.Lines(out.resolve("frames_cases.jsonl")); var records = new Out.Lines(out.resolve("frames.jsonl"))) {
            for (int c = 0; c < cases; c++) {
                String texture = c % 3 == 0 ? "clouds" : names.get(r.nextInt(names.size()));
                int range = c < 3 ? 128 : 2 + r.nextInt(texture.equals("clouds") ? 14 : 30);
                var status = c % 2 == 0 ? CloudStatus.FANCY : CloudStatus.FAST;
                float bottom = c % 4 == 0 ? 192.33F : r.nextFloat() * 300.0F - 40.0F;
                int color = (1 + r.nextInt(255)) << 24 | r.nextInt(1 << 24);
                double y = switch (c % 5) {
                    case 0 -> bottom + r.nextDouble() * 4.0;
                    case 1 -> bottom + 4.0 + r.nextDouble() * 20.0;
                    default -> bottom - r.nextDouble() * 150.0;
                };
                var camera = new Vec3(r.nextDouble() * 60000.0 - 30000.0, y, r.nextDouble() * 60000.0 - 30000.0);
                long gameTime = c % 4 == 1 ? r.nextInt(48000) : r.nextLong() >>> 20;
                float partial = r.nextFloat();
                String name = "frame-" + c;
                in.write(Out.map("case", name, "texture", texture, "color", color, "fancy", status == CloudStatus.FANCY, "bottom", bottom, "range", range,
                    "camera", new double[] {camera.x, camera.y, camera.z}, "game_time", gameTime, "partial", partial));
                records.write(frame(name, textures.get(texture), color, status, bottom, range, camera, gameTime, partial));
            }
        }
    }

    private static String bits(float v) {
        return Integer.toHexString(Float.floatToRawIntBits(v));
    }

    /** The vanilla map, and made-up ones (written as PNGs for the probe), loaded by CloudRenderer's own loader. */
    private static Map<String, CloudRenderer.TextureData> textures(Path dir) throws Exception {
        Files.createDirectories(dir);
        var pngs = new LinkedHashMap<String, byte[]>();
        try (var stream = Game.resources().open(Identifier.withDefaultNamespace("textures/environment/clouds.png"))) {
            pngs.put("clouds", stream.readAllBytes());
        }
        var r = new Random(6);
        int[] alphas = {0, 9, 10, 11, 128, 255};
        // The loader wraps east and west by the height: a map narrower than tall throws.
        int[][] sizes = {{8, 4}, {16, 16}, {32, 24}, {7, 5}};
        for (int[] size : sizes) {
            var image = new BufferedImage(size[0], size[1], BufferedImage.TYPE_INT_ARGB);
            for (int y = 0; y < size[1]; y++) {
                for (int x = 0; x < size[0]; x++) image.setRGB(x, y, alphas[r.nextInt(alphas.length)] << 24 | 0xFFFFFF);
            }
            var png = new ByteArrayOutputStream();
            ImageIO.write(image, "png", png);
            pngs.put(size[0] + "x" + size[1], png.toByteArray());
        }
        var out = new LinkedHashMap<String, CloudRenderer.TextureData>();
        Method prepare = CloudRenderer.class.getDeclaredMethod("prepare", ResourceManager.class, net.minecraft.util.profiling.ProfilerFiller.class);
        prepare.setAccessible(true);
        for (var e : pngs.entrySet()) {
            Files.write(dir.resolve(e.getKey() + ".png"), e.getValue());
            byte[] png = e.getValue();
            var manager = (ResourceManager) Proxy.newProxyInstance(ResourceManager.class.getClassLoader(), new Class<?>[] {ResourceManager.class}, (proxy, method, a) -> {
                if (!method.getName().equals("open")) throw new IllegalStateException("unexpected " + method);
                return new ByteArrayInputStream(png);
            });
            @SuppressWarnings("unchecked")
            var data = (Optional<CloudRenderer.TextureData>) prepare.invoke(Headless.allocate(CloudRenderer.class), manager, InactiveProfiler.INSTANCE);
            out.put(e.getKey(), data.orElseThrow());
        }
        return out;
    }

    /** Per cell, row by row: 0 if empty, else 16 | north 8 | east 4 | south 2 | west 1 empty. */
    private static List<Integer> cells(CloudRenderer.TextureData texture) {
        var out = new ArrayList<Integer>();
        for (long cell : texture.cells()) out.add(cell == 0 ? 0 : 16 | (int) (cell & 15));
        return out;
    }

    private static Map<String, Object> frame(String name, CloudRenderer.TextureData texture, int color, CloudStatus status, float bottom, int range, Vec3 camera,
        long gameTime, float partial) throws Exception {
        var renderer = Headless.allocate(CloudRenderer.class);
        Headless.set(renderer, "texture", texture);
        Headless.set(renderer, "needsRebuild", true);
        Method size = CloudRenderer.class.getDeclaredMethod("getSizeForCloudDistance", int.class);
        size.setAccessible(true);
        int radiusCells = (int) Math.ceil(range * 16 / 12.0F);
        var ubo = FakeRing.create(256);
        var utb = FakeRing.create((Integer) size.invoke(null, radiusCells));
        Headless.set(renderer, "ubo", ubo);
        Headless.set(renderer, "utb", utb);
        renderer.prepare(color, status, bottom, range, camera, gameTime, partial);
        int quads = (Integer) Headless.get(renderer, "quadCount");
        var faces = new ArrayList<String>();
        var bytes = utb.data;
        for (int q = 0; q < quads; q++) {
            int cx = bytes.get(q * 3), cz = bytes.get(q * 3 + 1), flags = Byte.toUnsignedInt(bytes.get(q * 3 + 2));
            cx = cx << 1 | (flags & 128) >> 7;
            cz = cz << 1 | (flags & 64) >> 6;
            faces.add(cx + " " + cz + " " + (flags & 7) + " " + ((flags & 16) != 0 ? 1 : 0) + " " + ((flags & 32) != 0 ? 1 : 0));
        }
        var record = Out.map("case", name, "faces", faces);
        if (quads != 0) {
            var u = ubo.data.order(ByteOrder.nativeOrder());
            var floats = new ArrayList<String>();
            for (int i : new int[] {0, 1, 2, 3, 4, 5, 6, 8, 9, 10}) floats.add(bits(u.getFloat(i * 4)));
            record.put("color", floats.subList(0, 4));
            record.put("offset", floats.subList(4, 7));
            record.put("cell_size", floats.subList(7, 10));
        }
        return record;
    }
}
