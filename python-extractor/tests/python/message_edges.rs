use models::{CommunicationProtocol, MessageDestinationKind, MessageRole};
use python_extractor::{
    extraction::{
        calls::{PythonCallStatement, extractor::CallsExtractor},
        extractor::{ExtractParams, Extractor},
        message_edges::{
            kafka::KafkaIdentificationStrategy, rabbitmq::RabbitMqIdentificationStrategy,
        },
    },
    s,
};

use crate::python::utils::get_tree;

#[test]
fn identifies_pika_publish_consume_queue_declare_and_bind() {
    let code = r#"
class RabbitMQPublisher:
    def publish(self, message):
        self._channel.queue_declare(queue=self.queue_name)
        self._channel.basic_publish(
            exchange='',
            routing_key=self.queue_name,
            body=json.dumps(message),
        )

class RabbitMQSubscriber:
    def connect(self):
        self.channel.queue_declare(queue=self.queue_name)
        self.channel.queue_bind(exchange='orders', queue=self.queue_name, routing_key='created')
        self.channel.basic_consume(queue=self.queue_name, on_message_callback=self.callback)
"#;
    let tree = get_tree(code);
    let calls = CallsExtractor
        .extract(ExtractParams::new(&tree, code))
        .into_iter()
        .map(PythonCallStatement::to_language_agnostic)
        .collect::<Vec<_>>();
    let strategy = RabbitMqIdentificationStrategy::new();
    let edges = calls
        .iter()
        .filter_map(|call| strategy.identify_message_edge(call, "rabbit.py"))
        .collect::<Vec<_>>();

    assert_eq!(edges.len(), 5);
    assert!(edges.iter().any(|edge| {
        edge.role == MessageRole::Producer
            && edge.destination_kind == MessageDestinationKind::Queue
            && edge.routing_key == Some(s!("self.queue_name"))
    }));
    assert!(edges.iter().any(|edge| {
        edge.role == MessageRole::Binding
            && edge.destination_kind == MessageDestinationKind::ExchangeRoutingKey
            && edge.exchange == Some(s!("orders"))
            && edge.routing_key == Some(s!("created"))
            && edge.queue == Some(s!("self.queue_name"))
    }));
    assert!(edges.iter().any(|edge| {
        edge.role == MessageRole::Consumer
            && edge.queue == Some(s!("self.queue_name"))
            && edge.handler == Some(s!("self.callback"))
    }));
}

#[test]
fn identifies_python_kafka_direct_clients() {
    let code = r#"
from confluent_kafka import Producer, Consumer
from kafka import KafkaConsumer
from aiokafka import AIOKafkaConsumer

def confluent(input_topic, output_topic):
    consumer = Consumer({})
    consumer.subscribe([input_topic])
    producer = Producer({})
    producer.produce(output_topic, b"payload")

def kafka_python(control_topic):
    consumer = KafkaConsumer(control_topic, group_id="logger")

async def aio(settings):
    consumer = AIOKafkaConsumer(settings.docs_topic, settings.signatures_topic)
    await producer.send_and_wait(settings.output_topic, value=b"payload")
"#;
    let tree = get_tree(code);
    let calls = CallsExtractor
        .extract(ExtractParams::new(&tree, code))
        .into_iter()
        .map(PythonCallStatement::to_language_agnostic)
        .collect::<Vec<_>>();
    let strategy = KafkaIdentificationStrategy::new();
    let edges = calls
        .iter()
        .flat_map(|call| strategy.identify_message_edges(call, "kafka.py"))
        .collect::<Vec<_>>();

    assert!(edges.iter().all(|edge| {
        edge.protocol == CommunicationProtocol::Kafka
            && edge.destination_kind == MessageDestinationKind::Topic
    }));
    assert!(edges.iter().any(|edge| {
        edge.role == MessageRole::Consumer && edge.topic.as_deref() == Some("input_topic")
    }));
    assert!(edges.iter().any(|edge| {
        edge.role == MessageRole::Producer && edge.topic.as_deref() == Some("output_topic")
    }));
    assert!(edges.iter().any(|edge| {
        edge.role == MessageRole::Consumer && edge.topic.as_deref() == Some("control_topic")
    }));
    assert!(edges.iter().any(|edge| {
        edge.role == MessageRole::Consumer && edge.topic.as_deref() == Some("settings.docs_topic")
    }));
    assert!(edges.iter().any(|edge| {
        edge.role == MessageRole::Consumer
            && edge.topic.as_deref() == Some("settings.signatures_topic")
    }));
    assert!(edges.iter().any(|edge| {
        edge.role == MessageRole::Producer && edge.topic.as_deref() == Some("settings.output_topic")
    }));
}
