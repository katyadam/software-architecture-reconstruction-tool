class Settings:
    QUEUE_TO_SUBSCRIBE: str = "videos_topic"
    RABBITMQ_HOST: str = "rabbitmq"


settings = Settings()


class RabbitMQSubscriber:
    def __init__(self, queue_name=None):
        self.queue_name = queue_name if queue_name else settings.QUEUE_TO_SUBSCRIBE
        self.channel = None

    def connect(self):
        self.channel.queue_declare(queue=self.queue_name)
        self.channel.basic_consume(queue=self.queue_name, on_message_callback=self.callback)

    def callback(self, ch, method, properties, body):
        print(body)
