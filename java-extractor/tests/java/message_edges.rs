use java_extractor::extraction::{
    calls::extractor::CallStatementsExtractor,
    extractor::Extractor,
    message_edges::{kafka::KafkaIdentificationStrategy, rabbitmq::RabbitMqIdentificationStrategy},
};
use models::{CommunicationProtocol, MessageDestinationKind, MessageRole};

use crate::java::utils::get_tree;

#[test]
fn identifies_java_rabbitmq_spring_calls_and_listener() {
    let code = r#"
import org.springframework.amqp.rabbit.annotation.RabbitListener;

class Messaging {
    void publish() {
        rabbitTemplate.convertAndSend(QUEUE_ORDERS, event);
        rabbitTemplate.convertAndSend(EXCHANGE, ROUTING_KEY, event);
        messageSender.sendMessage(QUEUE_BILLING, event);
        channel.basicPublish("", QUEUE_AUDIT, null, body);
        new Queue(QUEUE_ORDERS, true);
    }

    @RabbitListener(queues = QUEUE_ORDERS)
    void consume(String event) {
    }
}

class FlightUpdatedListener implements MessageHandler<FlightUpdated> {
    public void onMessage(FlightUpdated event) {
    }
}

class EventMapper {
    Object map() {
        return new FlightUpdated(id);
    }
}
"#;
    let tree = get_tree(code);
    let calls = CallStatementsExtractor.extract(code, &tree, "Messaging.java");
    let strategy = RabbitMqIdentificationStrategy::new();
    let mut edges = strategy.identify_from_calls(&calls, "Messaging.java");
    edges.extend(strategy.identify_from_annotations(code, &tree, "Messaging.java"));
    edges.extend(strategy.identify_from_message_handlers(code, &tree, "Messaging.java"));

    assert!(edges.iter().any(|edge| {
        edge.role == MessageRole::Producer
            && edge.destination_kind == MessageDestinationKind::Queue
            && edge.queue.as_deref() == Some("QUEUE_ORDERS")
    }));
    assert!(edges.iter().any(|edge| {
        edge.role == MessageRole::Producer
            && edge.destination_kind == MessageDestinationKind::ExchangeRoutingKey
            && edge.exchange.as_deref() == Some("EXCHANGE")
            && edge.routing_key.as_deref() == Some("ROUTING_KEY")
    }));
    assert!(edges.iter().any(|edge| {
        edge.role == MessageRole::Producer && edge.queue.as_deref() == Some("QUEUE_BILLING")
    }));
    assert!(edges.iter().any(|edge| {
        edge.role == MessageRole::Consumer && edge.queue.as_deref() == Some("QUEUE_ORDERS")
    }));
    assert!(edges.iter().any(|edge| {
        edge.role == MessageRole::QueueDeclaration && edge.queue.as_deref() == Some("QUEUE_ORDERS")
    }));
    assert!(edges.iter().any(|edge| {
        edge.role == MessageRole::Consumer && edge.queue.as_deref() == Some("FlightUpdated")
    }));
    assert!(edges.iter().any(|edge| {
        edge.role == MessageRole::Producer && edge.queue.as_deref() == Some("FlightUpdated")
    }));
}

#[test]
fn identifies_java_kafka_spring_calls_streams_and_listener() {
    let code = r##"
import org.apache.kafka.clients.admin.NewTopic;
import org.apache.kafka.clients.producer.ProducerRecord;
import org.apache.kafka.streams.StreamsBuilder;
import org.apache.kafka.streams.kstream.KStream;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.kafka.annotation.KafkaListener;
import org.springframework.kafka.core.KafkaTemplate;

class Messaging {
    private final KafkaTemplate<String, Order> template;
    private final String paymentTopic;

    Messaging(KafkaTemplate<String, Order> template, @Value("${topic.name.payment}") String paymentTopic) {
        this.template = template;
        this.paymentTopic = paymentTopic;
    }

    void publish(Order order) {
        template.send("orders", order.getId(), order);
        template.send(paymentTopic, order.getId(), order);
        producer.send(new ProducerRecord<>("audit", order));
    }

    @KafkaListener(id = "orders", topics = {"orders", "${topic.name.payment}"}, groupId = "test")
    void consume(Order order) {
    }

    KStream<String, Order> stream(StreamsBuilder builder) {
        KStream<String, Order> input = builder.stream("stock-orders");
        input.to("orders");
        return input;
    }

    NewTopic topic() {
        return TopicBuilder.name("orders").partitions(3).build();
    }
}
"##;
    let tree = get_tree(code);
    let calls = CallStatementsExtractor.extract(code, &tree, "Messaging.java");
    let strategy = KafkaIdentificationStrategy::new();
    let mut edges = strategy.identify_from_calls(&calls, code, "Messaging.java");
    edges.extend(strategy.identify_from_annotations(code, &tree, "Messaging.java"));

    assert!(edges.iter().any(|edge| {
        edge.protocol == CommunicationProtocol::Kafka
            && edge.role == MessageRole::Producer
            && edge.destination_kind == MessageDestinationKind::Topic
            && edge.topic.as_deref() == Some("orders")
    }));
    assert!(edges.iter().any(|edge| {
        edge.protocol == CommunicationProtocol::Kafka
            && edge.role == MessageRole::Producer
            && edge.topic.as_deref() == Some("topic.name.payment")
    }));
    assert!(edges.iter().any(|edge| {
        edge.protocol == CommunicationProtocol::Kafka
            && edge.role == MessageRole::Consumer
            && edge.topic.as_deref() == Some("orders")
    }));
    assert!(edges.iter().any(|edge| {
        edge.protocol == CommunicationProtocol::Kafka
            && edge.role == MessageRole::Consumer
            && edge.topic.as_deref() == Some("topic.name.payment")
    }));
    assert!(edges.iter().any(|edge| {
        edge.protocol == CommunicationProtocol::Kafka
            && edge.role == MessageRole::Consumer
            && edge.topic.as_deref() == Some("stock-orders")
    }));
    assert!(edges.iter().any(|edge| {
        edge.protocol == CommunicationProtocol::Kafka
            && edge.role == MessageRole::TopicDeclaration
            && edge.topic.as_deref() == Some("orders")
    }));

    let fluent_only = r#"
import org.apache.kafka.streams.StreamsBuilder;
import org.apache.kafka.streams.kstream.KStream;
class Streams {
    KStream<String, Order> stream(StreamsBuilder builder) {
        return builder.stream(stockTopic)
            .peek((key, value) -> log.info("event"))
            .to(orderTopic);
    }
}
"#;
    let fallback_edges = strategy.identify_stream_chain_outputs(&[], fluent_only, "Streams.java");
    assert!(fallback_edges.iter().any(|edge| {
        edge.protocol == CommunicationProtocol::Kafka
            && edge.role == MessageRole::Producer
            && edge.topic.as_deref() == Some("orderTopic")
    }));
}
