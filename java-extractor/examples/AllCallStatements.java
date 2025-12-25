import java.util.function.Consumer;
import java.util.function.Supplier;
import java.util.ArrayList;
import java.lang.reflect.Method;

public class CallPossibilities extends ParentClass {

    // 1. Constructor Call (Standard)
    CallPossibilities instance = new CallPossibilities();

    public CallPossibilities() {
        // 2. Constructor Chaining (calling another constructor in same class)
        this("Overloaded Call");
    }

    public CallPossibilities(String msg) {
        // 3. Super Constructor Call (calling parent constructor)
        super(); 
        System.out.println(msg);
    }

    public void demonstrateCalls() throws Exception {
        
        // 4. Local Instance Method Call
        internalMethod();

        // 5. Explicit Instance Call (using this)
        this.internalMethod();

        // 6. Superclass Method Call (calling overridden parent method)
        super.parentMethod();

        // 7. Static Method Call (via Class Name)
        StaticTarget.staticAction();

        // 8. Lambda Expression Call (defining a call to be executed later)
        Runnable r = () -> internalMethod();
        r.run();

        // 9. Method Reference: Static
        Consumer<String> printer = System.out::println;
        printer.accept("Method Reference Call");

        // 10. Method Reference: Constructor
        Supplier<ArrayList<String>> listSupplier = ArrayList::new;
        ArrayList<String> list = listSupplier.get();

        // 11. Reflection Call (Dynamic invocation by name)
        Method m = this.getClass().getMethod("internalMethod");
        m.invoke(this);
        
        // 12. Chained Method Call (Fluent API style)
        String result = "  hello  ".trim().toUpperCase().concat(" WORLD");
    }

    public void internalMethod() {
        System.out.println("Internal method executed.");
    }
}

class ParentClass {
    void parentMethod() {
        System.out.println("Parent method executed.");
    }
}

class StaticTarget {
    static void staticAction() {
        System.out.println("Static call executed.");
    }
}