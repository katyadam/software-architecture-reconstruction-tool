from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    as_url: str = ""

    model_config = SettingsConfigDict(env_file=".env", env_prefix="mds_", extra="ignore")
