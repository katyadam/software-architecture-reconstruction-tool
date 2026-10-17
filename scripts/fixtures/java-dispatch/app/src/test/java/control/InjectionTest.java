package control;

import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.test.context.junit.jupiter.SpringJUnitConfig;
import static org.junit.jupiter.api.Assertions.assertEquals;

@SpringJUnitConfig(QuoteConfiguration.class)
class InjectionTest {
    @Autowired
    private InjectedQuote quote;

    @Test
    void springInjection() {
        assertEquals(10, quote.amount());
    }
}
