import java.util.List;

public class CallStatementEdgeCases {

    // Edge case 1: Chained method calls — each link is a separate method_invocation node,
    // so both are captured as independent CallStatements.
    // inner: function_name = "input.trim()"
    // outer: function_name = "input.trim().toLowerCase()"
    public String normalize(String input) {
        return input.trim().toLowerCase();
    }

    // Edge case 2: Object creation expression — captured via object_creation_expression node,
    // not method_invocation. function_name = "StringBuilder(content)".
    public String buildMessage(String content) {
        StringBuilder sb = new StringBuilder(content);
        return sb.toString();
    }

    // Edge case 3: Method call inside a conditional guard — still captured even though it
    // is in a boolean expression position (not a statement position).
    public boolean isBlank(String s) {
        if (s.isEmpty()) {
            return true;
        }
        return false;
    }
}
