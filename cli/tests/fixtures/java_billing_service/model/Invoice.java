package billing.service.model;

import order.service.model.Order;

public class Invoice {
    private String id;
    private Order order;
    private double amount;

    public Invoice() {}

    public String getId() { return id; }
    public Order getOrder() { return order; }
    public double getAmount() { return amount; }
}
