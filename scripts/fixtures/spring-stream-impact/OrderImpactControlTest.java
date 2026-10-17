package pl.piomin.samples.kafka.order;

import org.junit.jupiter.api.Test;
import pl.piomin.samples.kafka.order.model.Order;
import static org.junit.jupiter.api.Assertions.assertEquals;

class OrderImpactControlTest {
    @Test
    void readsAmount() {
        assertEquals(0, new Order().getAmount());
    }

    @Test
    void readsAmountThroughFacade() {
        assertEquals(0, new OrderImpactFacade().currentAmount(new Order()));
    }

    @Test
    void readsId() {
        Order order = new Order();
        order.setId(42L);
        assertEquals(Long.valueOf(42L), order.getId());
    }
}
