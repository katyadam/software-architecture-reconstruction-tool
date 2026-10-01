public class OrderConsumer {
    @RabbitListener(queues = "orders.created")
    public void consume(String event) {
    }
}
