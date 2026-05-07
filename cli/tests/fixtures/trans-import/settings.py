from typing import Set

from pydantic import conint
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    connection_chunk_size: int = 1024000
    connection_limit_per_host: int = 100
    http_client_timeout: int = 300
    request_timeout: int = 300
    aaa_service_url: str = "http://aaa"
    app_service_url: str = "http://as"
    medical_data_service_url: str = "http://mds"
    job_execution_service_url: str = "http://jes"
    marketplace_service_url: str = "http://mps"
    id_mapper_url: str = ""
    cors_allow_credentials: bool = False
    cors_allow_origins: Set[str] = []
    api_v2_integration: str = ""
    api_v3_integration: str = ""
    debug: bool = False
    disable_openapi: bool = False
    root_path: str = ""
    idp_url: str = ""
    client_id: str = ""
    client_secret: str = ""
    organization_id: str = ""
    rsa_keys_directory: str = "./rsa/"
    frontend_token_exp: int = 86400  # 24h
    app_ui_frame_ancestors: str = ""
    app_ui_connect_src: str = ""
    frontend_csp_url: str = ""
    disable_csp_settings: bool = False
    generic_app_ui_v2_url: str = ""
    generic_app_ui_v3_url: str = ""
    daemon_poll_interval: int = 5
    daemon_job_ready_trials: int = 5
    daemon_zmq_port: int = 5556
    daemon_zmq_address: str = "tcp://workbench-daemon:5556"
    daemon_job_timeout: int = 86400  # 24h, a good value is equal to job_token_expire_seconds
    enable_eats_mode: bool = False
    job_token_expire_seconds: conint(gt=0) = 86400  # 24h
    disable_input_validation: bool = False
    disable_output_validation: bool = False
    disable_multi_user: bool = False
    enable_annot_case_data_partitioning: bool = False

    model_config = SettingsConfigDict(env_prefix="wbs_", env_file=".env", extra="ignore")
