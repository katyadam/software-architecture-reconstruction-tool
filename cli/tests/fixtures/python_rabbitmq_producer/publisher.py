import json


class Settings:
    QUEUE_NAME: str = "videos_topic"
    RABBITMQ_HOST: str = "rabbitmq"


settings = Settings()


class RabbitMQPublisher:
    def __init__(self, queue_name=None):
        self.queue_name = queue_name if queue_name else settings.QUEUE_NAME
        self._conn = None
        self._channel = None

    def connect(self):
        self._channel.queue_declare(queue=self.queue_name)

    def publish(self, message):
        self.connect()
        self._channel.basic_publish(
            exchange="",
            routing_key=self.queue_name,
            body=json.dumps(message),
        )
