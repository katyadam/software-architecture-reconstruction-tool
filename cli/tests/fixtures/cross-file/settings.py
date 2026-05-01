from typing import Set

from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    debug: bool = False
    cors_allow_credentials: bool = False
    cors_allow_origins: Set[str] = []
    api_integration: str = ""
    root_path: str = ""
    disable_openapi: bool = False
    http_client_timeout: int = 300
    request_timeout: int = 300
    connection_limit_per_host: int = 100
    connection_chunk_size: int = 1024000
    cds_url: str = ""
    es_url: str = ""
    js_url: str = ""
    as_url: str = ""
    hs_url: str = ""
    enable_storage_routes: bool = False
    enable_profiling: bool = False

    model_config = SettingsConfigDict(
        env_file=".env", env_prefix="mds_", extra="ignore")
