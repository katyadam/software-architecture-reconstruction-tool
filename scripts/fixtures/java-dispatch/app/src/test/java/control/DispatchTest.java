package control;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.assertEquals;

class DispatchTest {
    @Test
    void interfaceDispatch() {
        Quote quote = new OverriddenQuote();
        assertEquals(10, quote.amount());
    }

    @Test
    void baseImplementation() {
        Quote quote = new BaseQuote();
        assertEquals(20, quote.amount());
    }

    @Test
    void crossModule() {
        assertEquals(10, new SharedPrice().value());
    }
}
