package acacia.parity.clouds;

import java.lang.reflect.Field;

/** Objects built without their constructors (no window, no GPU); the weather harness has the same. */
final class Headless {
    private static final Object UNSAFE;

    static {
        try {
            Field field = Class.forName("sun.misc.Unsafe").getDeclaredField("theUnsafe");
            field.setAccessible(true);
            UNSAFE = field.get(null);
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    private Headless() {}

    static <T> T allocate(Class<T> type) throws Exception {
        return type.cast(UNSAFE.getClass().getMethod("allocateInstance", Class.class).invoke(UNSAFE, type));
    }

    static void set(Object owner, String name, Object value) throws Exception {
        field(owner.getClass(), name).set(owner, value);
    }

    static Object get(Object owner, String name) throws Exception {
        return field(owner.getClass(), name).get(owner);
    }

    private static Field field(Class<?> type, String name) throws NoSuchFieldException {
        for (Class<?> c = type; c != null; c = c.getSuperclass()) {
            try {
                Field field = c.getDeclaredField(name);
                field.setAccessible(true);
                return field;
            } catch (NoSuchFieldException ignored) {
                // a superclass's
            }
        }
        throw new NoSuchFieldException(name);
    }
}
