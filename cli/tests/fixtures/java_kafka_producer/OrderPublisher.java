package fixtures.kafka.producer;

import org.springframework.kafka.core.KafkaTemplate;

public class OrderPublisher {
    private final KafkaTemplate<String, String> kafkaTemplate;

    public OrderPublisher(KafkaTemplate<String, String> kafkaTemplate) {
        this.kafkaTemplate = kafkaTemplate;
    }

    public void publish(String payload) {
        kafkaTemplate.send(Topics.ORDER_EVENTS, "created", payload);
    }
}
