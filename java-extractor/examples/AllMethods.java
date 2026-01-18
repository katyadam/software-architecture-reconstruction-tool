package demo;

import java.io.IOException;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.Optional;

public abstract class MethodHeadersDemo<T extends Number> {

    /* ============================
     * Constructors
     * ============================ */

    public MethodHeadersDemo() {}

    protected MethodHeadersDemo(T value) {}

    MethodHeadersDemo(int x, String y) {} // package-private

    private MethodHeadersDemo(double hidden) {}

    /* ============================
     * Instance methods
     * ============================ */

    public void publicMethod() {}

    protected String protectedMethod(int a, long b) {
        return "";
    }

    void packagePrivateMethod(List<String> list) {}

    private int privateMethod(Map<String, Integer> map) {
        return map.size();
    }

    public final boolean finalMethod(Set<? extends T> values) {
        return !values.isEmpty();
    }

    public synchronized void synchronizedMethod() {}

    public native void nativeMethod();

    public abstract T abstractMethod(List<? super T> input);

    /* ============================
     * Static methods
     * ============================ */

    public static void publicStaticMethod() {}

    static int packagePrivateStaticMethod(int[] array) {
        return array.length;
    }

    private static Map<String, List<Integer>> privateStaticMethod(
            Map<String, List<Integer>> input) {
        return input;
    }

    protected static <K, V> Map<K, V> genericStaticMethod(
            List<K> keys,
            List<V> values) {
        return Map.of();
    }

    /* ============================
     * Generic methods
     * ============================ */

    public <E> E genericMethod(E value) {
        return value;
    }

    public <E extends Comparable<E>> List<E> boundedGenericMethod(
            List<E> list) {
        return list;
    }

    public <E extends Exception> void throwsGenericException()
            throws E {}

    /* ============================
     * Varargs / arrays
     * ============================ */

    public void varArgsMethod(String... args) {}

    public void mixedArrayMethod(
            int[] ints,
            List<String>[] listArray) {}

    /* ============================
     * Exceptions
     * ============================ */

    public void throwsMethod() throws IOException {}

    public void throwsMultipleMethod()
            throws IOException, IllegalStateException {}

    /* ============================
     * Annotations
     * ============================ */

    @Deprecated
    public Optional<String> annotatedMethod(
            @Deprecated List<@Deprecated String> input) {
        return Optional.empty();
    }

    /* ============================
     * Overloaded methods
     * ============================ */

    public void overloaded() {}

    public void overloaded(int x) {}

    public void overloaded(int x, String y) {}

    /* ============================
     * Inner interface (default + static)
     * ============================ */

    public interface InnerInterface {

        void interfaceMethod();

        default String defaultMethod(List<String> list) {
            return list.toString();
        }

        static int staticInterfaceMethod(Map<String, Integer> map) {
            return map.size();
        }
    }
}
