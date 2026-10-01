public class OrderPublisher {
    public void publish(Object event) {
        rabbitTemplate.convertAndSend(Queues.ORDER_QUEUE, event);
    }
}
