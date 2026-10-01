package fixtures.kafka.stock;

import org.springframework.kafka.annotation.KafkaListener;

public class StockConsumer {
    @KafkaListener(topics = Topics.ORDER_EVENTS, groupId = "stock")
    public void consume(String payload) {
    }
}
