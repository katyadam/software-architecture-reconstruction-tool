import logging

from pydantic import ValidationError

from .empaia_sender_auth import AioHttpClient, AuthSettings
from .settings import Settings
from .simple_cache import SimpleCache

settings = Settings()
aaa_url = settings.aaa_service_url.rstrip("/")
mps_url = settings.marketplace_service_url.rstrip("/")
mds_url = settings.medical_data_service_url.rstrip("/")
jes_url = settings.job_execution_service_url.rstrip("/")
idms_url = settings.id_mapper_url.rstrip("/") if settings.id_mapper_url is not None else None

logger = logging.getLogger("uvicorn")
logger.setLevel(logging.INFO)
if settings.debug:
    logger.setLevel(logging.DEBUG)

auth_settings = None
if settings.idp_url:
    try:
        auth_settings = AuthSettings(
            idp_url=settings.idp_url, client_id=settings.client_id, client_secret=settings.client_secret
        )
    except ValidationError as e:
        logger.info(f"Auth not configured: {e}")

http_client = AioHttpClient(
    logger=logger,
    auth_settings=auth_settings,
    chunk_size=settings.connection_chunk_size,
    timeout=settings.http_client_timeout,
    request_timeout=settings.request_timeout,
    connection_limit_per_host=settings.connection_limit_per_host,
)

# global ead cache
ead_cache = SimpleCache()

# v2 scope and job caches
v2_scope_ex_cache = SimpleCache()
v2_ex_job_cache = SimpleCache()

# v3 scope and job caches
v3_scope_ex_cache = SimpleCache()
v3_ex_job_cache = SimpleCache()
v3_ex_scopes_cache = SimpleCache()
v3_scope_case_cache = SimpleCache()

# TODO: Get default CSP from definitions repo or serve by generic app UI container?
default_generic_app_ui_config = {"csp": {"style_src": {"unsafe_inline": True}, "font_src": {"unsafe_inline": True}}}
