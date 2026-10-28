package acacia.parity;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import java.io.BufferedWriter;
import java.io.IOException;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Locale;
import java.util.TreeMap;
import java.util.zip.GZIPOutputStream;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.Property;

/** What oracles write: JSON (nulls kept), JSON lines, and the names both sides agree on. */
public final class Out {
    public static final Gson JSON = new GsonBuilder().serializeNulls().disableHtmlEscaping().create();

    private Out() {}

    /** A sorted map from alternating keys and values. */
    public static TreeMap<String, Object> map(Object... pairs) {
        var out = new TreeMap<String, Object>();
        for (int i = 0; i < pairs.length; i += 2) out.put((String) pairs[i], pairs[i + 1]);
        return out;
    }

    public static void json(Path path, Object value) throws IOException {
        Files.createDirectories(path.toAbsolutePath().getParent());
        var stream = Files.newOutputStream(path);
        var raw = path.toString().endsWith(".gz") ? new GZIPOutputStream(stream) : stream;
        try (var writer = new BufferedWriter(new OutputStreamWriter(raw, StandardCharsets.UTF_8))) {
            JSON.toJson(value, writer);
        }
    }

    /** One JSON value per line, for record-per-case outputs (paritylib.compare.jsonl reads them). */
    public static final class Lines implements AutoCloseable {
        private final BufferedWriter writer;

        public Lines(Path path) throws IOException {
            Files.createDirectories(path.toAbsolutePath().getParent());
            writer = Files.newBufferedWriter(path, StandardCharsets.UTF_8);
        }

        public void write(Object value) throws IOException {
            writer.write(JSON.toJson(value));
            writer.newLine();
        }

        @Override
        public void close() throws IOException {
            writer.close();
        }
    }

    /** `minecraft:oak_slab[type=double,waterlogged=false]`, properties sorted by name. */
    public static String stateKey(BlockState state) {
        var name = BuiltInRegistries.BLOCK.getKey(state.getBlock()).toString();
        var pairs = new ArrayList<String>();
        for (Property<?> property : state.getProperties()) pairs.add(property.getName() + "=" + value(state, property));
        if (pairs.isEmpty()) return name;
        pairs.sort(null);
        return name + "[" + String.join(",", pairs) + "]";
    }

    private static <T extends Comparable<T>> String value(BlockState state, Property<T> property) {
        return property.getName(state.getValue(property));
    }

    /** `rrggbb` of an ARGB colour. */
    public static String rgb(int argb) {
        return String.format(Locale.ROOT, "%06x", argb & 0xFFFFFF);
    }

    /** `aarrggbb`. */
    public static String argb(int argb) {
        return String.format(Locale.ROOT, "%08x", argb);
    }
}
