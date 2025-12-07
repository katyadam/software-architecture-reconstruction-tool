import java.util.List;
import java.util.Map;

public class AllFieldTypes extends SomeClass implements SomeInterface {

    // Access modifiers
    public String publicField;
    protected int protectedField;
    private boolean privateField;
    String packageField; // default

    // Static
    public static int staticField;

    // Final
    public final String FINAL_FIELD = "constant";
    private final int finalInitializedInConstructor;

    // Transient / Volatile
    private transient String tempData;
    private volatile boolean running;

    // Array and Collection
    private int[] numbers;
    private List<String> names;
    private Map<String, Integer> scores;

    // Constructor for final field
    public AllFieldTypes(int value) {
        this.finalInitializedInConstructor = value;
    }
}