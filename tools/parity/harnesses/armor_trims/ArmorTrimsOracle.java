package acacia.parity.armor_trims;

import acacia.parity.Game;
import acacia.parity.Out;
import com.google.gson.JsonParser;
import com.mojang.blaze3d.platform.NativeImage;
import com.mojang.serialization.JsonOps;
import java.io.InputStreamReader;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.*;
import net.minecraft.client.renderer.entity.layers.EquipmentLayerRenderer;
import net.minecraft.client.resources.metadata.texture.PaletteMetadataSection;
import net.minecraft.client.resources.model.EquipmentClientInfo;
import net.minecraft.client.resources.palette.Palette;
import net.minecraft.client.resources.palette.PaletteMapping;
import net.minecraft.client.resources.palette.PalettedTextureManager;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.equipment.trim.ArmorTrim;

/**
 * Armour trim textures (docs/parity.md, harness armor_trims): for every equipment asset with
 * humanoid layers, both trimmed layer types, every pattern and material, the texture
 * EquipmentLayerRenderer's TrimTextureKey asks for (its own code, by reflection, against a
 * PalettedTextureManager that only records the ask) and that texture as PalettedTextureManager
 * makes it (the base texture's PaletteMetadataSection, PaletteMapping, NativeImage.mappedCopy),
 * as an FNV-1a 64 hash of its RGBA bytes. args[0]/trims.jsonl.
 */
public final class ArmorTrimsOracle {
    /** Records getOrPrepare's arguments; allocated without its constructor, which wants a GPU. */
    static final class Recorder extends PalettedTextureManager {
        Recorder() {
            super(null, null);
        }

        Identifier base, palette;

        @Override
        public PalettedTextureManager.Handle getOrPrepare(Identifier baseTexture, Identifier paletteId) {
            base = baseTexture;
            palette = paletteId;
            return new PalettedTextureManager.Handle() {
                @Override
                public Identifier textureLocation() {
                    return baseTexture;
                }

                @Override
                public float getU(float offset) {
                    return offset;
                }

                @Override
                public float getV(float offset) {
                    return offset;
                }
            };
        }
    }

    public static void main(String[] args) throws Exception {
        Game.init();
        var resources = Game.resources();
        var palettes = Palette.listAndLoad(resources, Runnable::run).join();
        var unsafe = Class.forName("sun.misc.Unsafe").getDeclaredField("theUnsafe");
        unsafe.setAccessible(true);
        var u = unsafe.get(null);
        var recorder = (Recorder) u.getClass().getMethod("allocateInstance", Class.class).invoke(u, Recorder.class);
        Class<?> keyClass = Arrays.stream(EquipmentLayerRenderer.class.getDeclaredClasses()).filter(c -> c.getSimpleName().equals("TrimTextureKey")).findFirst().orElseThrow();
        var constructor = keyClass.getDeclaredConstructor(ArmorTrim.class, EquipmentClientInfo.LayerType.class, EquipmentClientInfo.class);
        constructor.setAccessible(true);
        Method prepare = keyClass.getDeclaredMethod("getOrPrepareTexture", PalettedTextureManager.class);
        prepare.setAccessible(true);
        var materials = Game.world().lookupOrThrow(Registries.TRIM_MATERIAL).listElements().toList();
        var patterns = Game.world().lookupOrThrow(Registries.TRIM_PATTERN).listElements().toList();
        var types = List.of(EquipmentClientInfo.LayerType.HUMANOID, EquipmentClientInfo.LayerType.HUMANOID_LEGGINGS);
        try (var out = new Out.Lines(Path.of(args[0]).resolve("trims.jsonl"))) {
            var assets = new TreeMap<>(resources.listResources("equipment", p -> p.getPath().endsWith(".json")));
            for (var asset : assets.entrySet()) {
                EquipmentClientInfo info;
                try (var reader = new InputStreamReader(asset.getValue().open(), StandardCharsets.UTF_8)) {
                    info = EquipmentClientInfo.CODEC.parse(JsonOps.INSTANCE, JsonParser.parseReader(reader)).getOrThrow();
                }
                if (info.getLayers(EquipmentClientInfo.LayerType.HUMANOID).isEmpty()) continue;
                String name = asset.getKey().getPath().replace("equipment/", "").replace(".json", "");
                for (var type : types) {
                    for (var pattern : patterns) {
                        for (var material : materials) {
                            var kase = name + "|" + type.getSerializedName() + "|" + pattern.key().identifier().getPath() + "|" + material.key().identifier().getPath();
                            var record = new TreeMap<String, Object>();
                            record.put("case", kase);
                            try {
                                recorder.base = null;
                                recorder.palette = null;
                                var key = constructor.newInstance(new ArmorTrim(material, pattern), type, info);
                                var handle = (PalettedTextureManager.Handle) prepare.invoke(key, recorder);
                                Identifier base = recorder.base != null ? recorder.base : handle.textureLocation();
                                record.put("base", base.toString());
                                record.put("palette", recorder.palette == null ? null : recorder.palette.toString());
                                record.put("hash", hash(resources, recorder.base == null ? null : base, recorder.palette, palettes, base));
                            } catch (Throwable error) {
                                record.put("error", String.valueOf(error.getCause() != null ? error.getCause() : error));
                            }
                            out.write(record);
                        }
                    }
                }
            }
        }
    }

    /** The texture as the game makes it: paletted (`paletted` base id) or the unpaletted location. */
    private static String hash(net.minecraft.server.packs.resources.ResourceManager resources, Identifier paletted, Identifier paletteId,
                               Map<Identifier, Palette> palettes, Identifier location) throws Exception {
        var file = paletted == null ? location : paletted.withPath(p -> "textures/" + p + ".png");
        var resource = resources.getResourceOrThrow(file);
        try (var image = NativeImage.read(resource.open())) {
            var mapping = PaletteMapping.NONE;
            if (paletted != null) {
                var meta = resource.metadata().getSection(PaletteMetadataSection.TYPE).orElseThrow();
                mapping = PaletteMapping.create(palettes.get(meta.basePalette()), palettes.get(paletteId));
            }
            try (var mapped = image.mappedCopy(mapping)) {
                long h = 0xcbf29ce484222325L;
                for (int argb : mapped.getPixels()) {
                    for (int shift : new int[] {16, 8, 0, 24}) {
                        h ^= (argb >>> shift) & 0xFF;
                        h *= 0x100000001b3L;
                    }
                }
                return Long.toHexString(h);
            }
        }
    }
}
