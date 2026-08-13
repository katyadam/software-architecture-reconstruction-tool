package fixtures.kafka.payment;

import org.springframework.kafka.annotation.KafkaListener;

public class PaymentConsumer {
    @KafkaListener(topics = Topics.ORDER_EVENTS, groupId = "payment")
    public void consume(String payload) {
    }
}
