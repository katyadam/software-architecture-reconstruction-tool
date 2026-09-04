TOPIC = "orders.created"


def consume_orders():
    consumer.subscribe([TOPIC])
