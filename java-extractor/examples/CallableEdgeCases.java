import java.util.List;

public class CallableEdgeCases {

    // Edge case 1: Annotated parameters — @RequestBody / @PathVariable are stripped from
    // the extracted Parameter.datatype and Parameter.name fields, but remain in the
    // callable's name string (which is built from the raw params text).
    public Object create(@RequestBody Object body, @PathVariable String id) {
        return null;
    }

    // Edge case 2: Generic method with bounded type parameter — the type bound
    // <T extends Comparable<T>> is NOT reflected in the callable name or return type;
    // only the bare return type "T" and param type "List<T>" are captured.
    public <T extends Comparable<T>> T findMax(List<T> items) {
        return items.get(0);
    }

    // Edge case 3: Methods in a static inner class — captured with the inner class
    // as their namespace (Namespace::Class("Validator")), not the outer class.
    public static class Validator {

        public boolean validate(String input) {
            return input != null && !input.isEmpty();
        }

        public void report(String message) {
            System.out.println(message);
        }
    }
}
