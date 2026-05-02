import asyncio
from contextlib import asynccontextmanager

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

from ...python_chained_endpoints_example import __version__ as version
from .api.v2 import add_routes_v2, add_routes_v2_frontends, add_routes_v2_scopes
from .api.v3 import add_routes_v3, add_routes_v3_frontends, add_routes_v3_scopes
from .daemon_tick import make_daemon_tick
from .late_init import LateInit
from .singletons import http_client, logger, settings

openapi_url = "/openapi.json"
if settings.disable_openapi:
    openapi_url = ""

late_init = LateInit()


@asynccontextmanager
async def lifespan(fastapi_app: FastAPI):
    late_init.daemon_tick = make_daemon_tick(
        address=settings.daemon_zmq_address, logger=logger)
    asyncio.create_task(http_client.update_token_routine())
    yield


app = FastAPI(
    debug=settings.debug,
    title="Workbench API",
    version=version,
    description="Workbench API",
    root_path=settings.root_path,
    openapi_url=openapi_url,
    lifespan=lifespan,
)


@app.get("/alive", tags=["Private"])
async def _():
    return {"status": "ok", "version": version}


app_v2 = FastAPI(openapi_url=openapi_url,
                 title="Workbench V2", version=version)
app_v2_scopes = FastAPI(openapi_url=openapi_url,
                        title="Workbench V2 Scopes", version=version)
app_v2_frontends = FastAPI(openapi_url=openapi_url,
                           title="Workbench V2 Frondends", version=version)
app_v3 = FastAPI(openapi_url=openapi_url,
                 title="Workbench V3", version=version)
app_v3_scopes = FastAPI(openapi_url=openapi_url,
                        title="Workbench V3 Scopes", version=version)
app_v3_frontends = FastAPI(openapi_url=openapi_url,
                           title="Workbench V3 Frontends", version=version)


if settings.cors_allow_origins:
    for app_obj in [
        app_v2,
        app_v3,
    ]:
        app_obj.add_middleware(
            CORSMiddleware,
            allow_origins=settings.cors_allow_origins,
            allow_credentials=settings.cors_allow_credentials,
            allow_methods=["*"],
            allow_headers=["*"],
        )

for app_obj in [
    app_v2_scopes,
    app_v2_frontends,
    app_v3_scopes,
    app_v3_frontends,
]:
    app_obj.add_middleware(
        CORSMiddleware,
        allow_origins=["null"],
        allow_credentials=False,
        allow_methods=["*"],
        allow_headers=["*"],
    )

add_routes_v2(app_v2, late_init)
add_routes_v2_scopes(app_v2_scopes, late_init)
add_routes_v2_frontends(app_v2_frontends, late_init)
add_routes_v3(app_v3, late_init)
add_routes_v3_scopes(app_v3_scopes, late_init)
add_routes_v3_frontends(app_v3_frontends, late_init)

app.mount("/v2/frontends", app_v2_frontends)  # must be mounted before app_v2
app.mount("/v2/scopes", app_v2_scopes)  # must be mounted before app_v2
app.mount("/v2", app_v2)
app.mount("/v3/frontends", app_v3_frontends)
app.mount("/v3/scopes", app_v3_scopes)
app.mount("/v3", app_v3)
