import logging

from .api_integrations import get_api_integration
from .empaia_sender_auth import AioHttpClient
from settings import Settings

settings = Settings()

logger = logging.getLogger("uvicorn")
logger.setLevel(logging.INFO)
if settings.debug:
    logger.setLevel(logging.DEBUG)

api_integration = get_api_integration(settings, logger)
http_client = AioHttpClient(
    logger=logger,
    timeout=settings.http_client_timeout,
    request_timeout=settings.request_timeout,
    chunk_size=settings.connection_chunk_size,
    connection_limit_per_host=settings.connection_limit_per_host,
)
