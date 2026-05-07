from ...models.utils.access_token_tools import AccessTokenTools
from ...simple_cache import SimpleCache
from ...singletons import http_client, logger, settings
from .integrations import get_api_integration
from .validation_frontend import FrontendValidation
from .validation_scope import ScopeValidation

api_integration = get_api_integration(settings=settings, logger=logger, http_client=http_client)
scope_validation = ScopeValidation(api_integration)
access_token_tools = AccessTokenTools(settings.rsa_keys_directory)
frontend_validation = FrontendValidation(access_token_tools)
app_ui_csp_cache = SimpleCache()
