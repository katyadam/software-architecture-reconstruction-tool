package java_order_service.controller;

import java_order_service.model.Order;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RestController;
import java.util.List;

@RestController
public class OrderController {

    @GetMapping("/api/v1/orders/")
    public List<Order> getAllOrders() {
        return List.of();
    }

    @GetMapping("/api/v1/orders/{id}")
    public Order getOrderById(@PathVariable Long id) {
        return new Order();
    }

    @PostMapping("/api/v1/orders/")
    public Order createOrder(@RequestBody Order order) {
        return order;
    }

    @DeleteMapping("/api/v1/orders/{id}")
    public void deleteOrder(@PathVariable Long id) {
    }
}
