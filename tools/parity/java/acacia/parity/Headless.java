package acacia.parity;

import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.ByteBuffer;
import java.util.LinkedHashMap;
import java.util.Map;
import sun.misc.Unsafe;

/**
 * Game objects whose constructors need a window, a server or a GPU: allocated bare (no
 * constructor runs), their fields set by name, their private members called. A subclass
 * allocated this way overrides only accessors, so the rule under test stays the game's code.
 */
public final class Headless {
    private static final Unsafe UNSAFE;

    static {
        try {
            Field f = Unsafe.class.getDeclaredField("theUnsafe");
            f.setAccessible(true);
            UNSAFE = (Unsafe) f.get(null);
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    private Headless() {}

    public static <T> T allocate(Class<T> type) {
        try {
            return type.cast(UNSAFE.allocateInstance(type));
        } catch (InstantiationException e) {
            throw new IllegalStateException(e);
        }
    }

    private static Field field(Class<?> type, String name) {
        for (Class<?> c = type; c != null; c = c.getSuperclass()) {
            try {
                Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f;
            } catch (NoSuchFieldException ignored) {
            }
        }
        throw new IllegalArgumentException(type.getName() + " has no field " + name);
    }

    /** Final fields too: Unsafe writes past `final`. */
    public static void set(Object target, String name, Object value) {
        Field f = field(target.getClass(), name);
        long offset = UNSAFE.objectFieldOffset(f);
        if (f.getType() == int.class) UNSAFE.putInt(target, offset, (Integer) value);
        else if (f.getType() == float.class) UNSAFE.putFloat(target, offset, (Float) value);
        else if (f.getType() == long.class) UNSAFE.putLong(target, offset, (Long) value);
        else if (f.getType() == boolean.class) UNSAFE.putBoolean(target, offset, (Boolean) value);
        else UNSAFE.putObject(target, offset, value);
    }

    public static Object get(Object target, String name) {
        try {
            return field(target.getClass(), name).get(target);
        } catch (IllegalAccessException e) {
            throw new IllegalStateException(e);
        }
    }

    public static void setStatic(Class<?> type, String name, Object value) {
        try {
            field(type, name).set(null, value);
        } catch (IllegalAccessException e) {
            throw new IllegalStateException(e);
        }
    }

    public static Object getStatic(Class<?> type, String name) {
        try {
            return field(type, name).get(null);
        } catch (IllegalAccessException e) {
            throw new IllegalStateException(e);
        }
    }

    /** A declared (possibly private) method of `type`; `target` null for a static one. */
    public static Object call(Class<?> type, Object target, String name, Class<?>[] parameters, Object... args) {
        try {
            Method m = type.getDeclaredMethod(name, parameters);
            m.setAccessible(true);
            return m.invoke(target, args);
        } catch (InvocationTargetException e) {
            throw new IllegalStateException(name + " threw", e.getCause());
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
    }

    /** An interface whose methods all return their type's zero, `null` for objects. */
    public static <T> T inert(Class<T> type) {
        return type.cast(Proxy.newProxyInstance(type.getClassLoader(), new Class<?>[] {type}, (proxy, method, args) -> zero(method.getReturnType())));
    }

    static Object zero(Class<?> type) {
        if (type == boolean.class) return false;
        if (type == int.class) return 0;
        if (type == long.class) return 0L;
        if (type == float.class) return 0.0F;
        if (type == double.class) return 0.0;
        return null;
    }

    /**
     * Installs a GPU device that keeps every buffer created from data, by label, so CPU-side
     * mesh builders (SkyRenderer's) can be read back. Returns the label-to-bytes map it fills.
     */
    public static Map<String, ByteBuffer> recordGpuBuffers() throws Exception {
        Map<String, ByteBuffer> buffers = new LinkedHashMap<>();
        Class<?> deviceType = Class.forName("com.mojang.renderpearl.api.device.GpuDevice");
        Class<?> bufferType = Class.forName("com.mojang.renderpearl.api.buffers.GpuBuffer");
        Object device = Proxy.newProxyInstance(deviceType.getClassLoader(), new Class<?>[] {deviceType}, (proxy, method, args) -> {
            if (method.getName().equals("createBuffer") && args != null && args[2] instanceof ByteBuffer data) {
                @SuppressWarnings("unchecked")
                var label = ((java.util.function.Supplier<String>) args[0]).get();
                var copy = ByteBuffer.allocate(data.remaining()).order(data.order());
                copy.put(data.duplicate()).flip();
                buffers.put(label, copy);
                long size = copy.remaining();
                return Proxy.newProxyInstance(bufferType.getClassLoader(), new Class<?>[] {bufferType},
                    (p, m, a) -> m.getName().equals("size") ? size : zero(m.getReturnType()));
            }
            return zero(method.getReturnType());
        });
        setStatic(Class.forName("com.mojang.blaze3d.systems.RenderSystem"), "DEVICE", device);
        return buffers;
    }
}
