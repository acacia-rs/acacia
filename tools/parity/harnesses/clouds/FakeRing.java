package acacia.parity.clouds;

import com.mojang.renderpearl.api.buffers.GpuBuffer;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
import java.lang.reflect.Proxy;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import net.minecraft.client.renderer.MappableRingBuffer;

/** A MappableRingBuffer with no GPU behind it: one direct buffer, mapped as is, never rotated. */
final class FakeRing extends MappableRingBuffer {
    ByteBuffer data;
    private int size;
    private GpuBuffer buffer;

    private FakeRing() {
        super(null, 0, 0);
    }

    static FakeRing create(int size) throws Exception {
        FakeRing ring = Headless.allocate(FakeRing.class);
        ring.size = size;
        ring.data = ByteBuffer.allocateDirect(size).order(ByteOrder.nativeOrder());
        ring.buffer = (GpuBuffer) Proxy.newProxyInstance(GpuBuffer.class.getClassLoader(), new Class<?>[] {GpuBuffer.class}, (proxy, method, a) ->
            switch (method.getName()) {
                case "size" -> (long) ring.size;
                case "isClosed" -> false;
                case "map" -> new GpuBufferSlice.MappedView(new GpuBufferSlice((GpuBuffer) proxy, 0, ring.size), ring.data.clear(), () -> {});
                default -> throw new IllegalStateException("unexpected " + method);
            });
        return ring;
    }

    @Override
    public int size() {
        return size;
    }

    @Override
    public GpuBuffer currentBuffer() {
        return buffer;
    }

    @Override
    public void rotate() {}

    @Override
    public void close() {}
}
