TOPIC = "orders.created"


def publish_order(event):
    producer.send(TOPIC, value=event)
