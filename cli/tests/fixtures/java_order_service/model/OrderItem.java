package java_order_service.model;

public class OrderItem {
    private String productName;
    private int quantity;
    private double price;

    public OrderItem() {}

    public String getProductName() { return productName; }
    public int getQuantity() { return quantity; }
    public double getPrice() { return price; }
}
