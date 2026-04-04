package java_order_service.model;

import java.util.List;

public class Order {
    private Long id;
    private String name;
    private List<OrderItem> items;

    public Order() {}

    public Long getId() { return id; }
    public String getName() { return name; }
    public List<OrderItem> getItems() { return items; }
}
