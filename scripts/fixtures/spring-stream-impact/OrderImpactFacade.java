package pl.piomin.samples.kafka.order;

import pl.piomin.samples.kafka.order.model.Order;

public class OrderImpactFacade {
    public int currentAmount(Order order) {
        return order.getAmount();
    }
}
