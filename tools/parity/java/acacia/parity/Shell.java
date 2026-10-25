package acacia.parity;

import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import sun.misc.Unsafe;

/**
 * Reflection for stand-ins: objects allocated without running their constructor (a Minecraft, a
 * TextureAtlas whose constructor would reach for the GPU) and private or final fields set by name.
 * A shell answers only what its fields hold; set what the code under test reads.
 */
@SuppressWarnings({"removal", "deprecation"})
public final class Shell {
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

    private Shell() {}

    /** An instance whose constructor never ran: every field zero or null. */
    @SuppressWarnings("unchecked")
    public static <T> T alloc(Class<T> type) {
        try {
            return (T) UNSAFE.allocateInstance(type);
        } catch (InstantiationException e) {
            throw new IllegalStateException(e);
        }
    }

    /** The field `name` declared on `owner` or a superclass. */
    public static Field field(Class<?> owner, String name) {
        for (Class<?> k = owner; k != null; k = k.getSuperclass()) {
            try {
                Field f = k.getDeclaredField(name);
                f.setAccessible(true);
                return f;
            } catch (NoSuchFieldException e) {
                // look further up
            }
        }
        throw new IllegalArgumentException("no field " + name + " on " + owner.getName());
    }

    /** Sets a field, final or not, on `target` (or a static one when target is a Class). */
    public static void set(Object target, String name, Object value) {
        boolean statik = target instanceof Class<?>;
        Field f = field(statik ? (Class<?>) target : target.getClass(), name);
        Object base = statik || Modifier.isStatic(f.getModifiers()) ? UNSAFE.staticFieldBase(f) : target;
        long offset = Modifier.isStatic(f.getModifiers()) ? UNSAFE.staticFieldOffset(f) : UNSAFE.objectFieldOffset(f);
        Class<?> t = f.getType();
        if (t == int.class) UNSAFE.putInt(base, offset, (Integer) value);
        else if (t == boolean.class) UNSAFE.putBoolean(base, offset, (Boolean) value);
        else if (t == float.class) UNSAFE.putFloat(base, offset, (Float) value);
        else if (t == double.class) UNSAFE.putDouble(base, offset, (Double) value);
        else if (t == long.class) UNSAFE.putLong(base, offset, (Long) value);
        else if (t.isPrimitive()) throw new IllegalArgumentException("unsupported field type " + t);
        else UNSAFE.putObject(base, offset, value);
    }

    @SuppressWarnings("unchecked")
    public static <T> T get(Object target, String name) {
        boolean statik = target instanceof Class<?>;
        try {
            return (T) field(statik ? (Class<?>) target : target.getClass(), name).get(statik ? null : target);
        } catch (IllegalAccessException e) {
            throw new IllegalStateException(e);
        }
    }

    /** Calls a private method `name` of `owner` whose parameters are `types`. */
    @SuppressWarnings("unchecked")
    public static <T> T call(Class<?> owner, Object target, String name, Class<?>[] types, Object... args) {
        try {
            Method m = owner.getDeclaredMethod(name, types);
            m.setAccessible(true);
            return (T) m.invoke(target, args);
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(owner.getSimpleName() + "." + name, e);
        }
    }

    /** Constructs `type` through its only (or first) declared constructor, private ones included. */
    @SuppressWarnings("unchecked")
    public static <T> T construct(Class<T> type, Object... args) {
        try {
            Constructor<?> c = type.getDeclaredConstructors()[0];
            c.setAccessible(true);
            return (T) c.newInstance(args);
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(type.getName(), e);
        }
    }
}
