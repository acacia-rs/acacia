package acacia.parity.gui;

import java.lang.reflect.Field;
import java.lang.reflect.Method;

/**
 * Builds the game's objects without running their constructors (no window, no GPU): Unsafe
 * allocation and field writes by name. Adapted from Horizon's parity oracles (MIT, (c) Ludovic Marie).
 */
public final class Stub {
    private static final Object UNSAFE;
    private static final Method ALLOCATE;

    static {
        try {
            var f = Class.forName("sun.misc.Unsafe").getDeclaredField("theUnsafe");
            f.setAccessible(true);
            UNSAFE = f.get(null);
            ALLOCATE = UNSAFE.getClass().getMethod("allocateInstance", Class.class);
        } catch (Exception e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    private Stub() {}

    @SuppressWarnings("unchecked")
    public static <T> T alloc(Class<T> type) throws Exception {
        return (T) ALLOCATE.invoke(UNSAFE, type);
    }

    public static Field field(Class<?> type, String name) throws NoSuchFieldException {
        for (Class<?> k = type; k != null; k = k.getSuperclass()) {
            try {
                Field f = k.getDeclaredField(name);
                f.setAccessible(true);
                return f;
            } catch (NoSuchFieldException e) {
                // look in the superclass
            }
        }
        throw new NoSuchFieldException(name + " on " + type);
    }

    public static Object get(Object target, Class<?> owner, String name) throws Exception {
        return field(owner, name).get(target);
    }

    /** Writes even a final field. */
    public static void put(Object target, Class<?> owner, String name, Object value) throws Exception {
        Field f = field(owner, name);
        var u = UNSAFE.getClass();
        long offset = (long) u.getMethod("objectFieldOffset", Field.class).invoke(UNSAFE, f);
        Class<?> t = f.getType();
        String setter = t == int.class ? "putInt" : t == boolean.class ? "putBoolean" : t == float.class ? "putFloat"
            : t == long.class ? "putLong" : t == double.class ? "putDouble" : "putObject";
        Class<?> param = t.isPrimitive() ? t : Object.class;
        u.getMethod(setter, Object.class, long.class, param).invoke(UNSAFE, target, offset, value);
    }

    public static void putStatic(Class<?> owner, String name, Object value) throws Exception {
        Field f = field(owner, name);
        var u = UNSAFE.getClass();
        Object base = u.getMethod("staticFieldBase", Field.class).invoke(UNSAFE, f);
        long offset = (long) u.getMethod("staticFieldOffset", Field.class).invoke(UNSAFE, f);
        u.getMethod("putObject", Object.class, long.class, Object.class).invoke(UNSAFE, base, offset, value);
    }

    public static Method method(Class<?> owner, String name, Class<?>... params) throws Exception {
        Method m = owner.getDeclaredMethod(name, params);
        m.setAccessible(true);
        return m;
    }
}
