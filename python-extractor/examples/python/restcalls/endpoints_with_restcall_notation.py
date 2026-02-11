# pylint: disable=function-redefined

from fastapi import Header

from .. import params
from ..connectors import cds_connector
from ..custom_models.cases import Case, CaseList
from ..custom_models.slides import SlideList
from ..singletons import api_integration


def add_routes(app, late_init):
    @app.get(
        "/cases",
        tags=["cases"],
        status_code=200,
        responses={200: {"model": CaseList}},
    )
    async def _(
        skip: int = None,
        limit: int = None,
        user_id: str = Header(...),
        payload=api_integration.global_depends(),
    ):
        cases_filter = await api_integration.user_cases_filter_hook(user_id=user_id, auth_payload=payload)
        return await cds_connector.fetch_cases(cases_filter=cases_filter, skip=skip, limit=limit)

    @app.get(
        "/cases/{case_id}",
        tags=["cases"],
        status_code=200,
        responses={200: {"model": Case}},
    )
    async def _(
        user_id: str = Header(...),
        case_id: str = params.case_id,
        payload=api_integration.global_depends(),
    ):
        await api_integration.user_case_hook(user_id=user_id, case_id=case_id, auth_payload=payload)
        case = await cds_connector.fetch_case(case_id=case_id)
        return case

    @app.get(
        "/cases/{case_id}/slides",
        tags=["cases"],
        status_code=200,
        responses={200: {"model": SlideList}},
    )
    async def _(
        skip: int = None,
        limit: int = None,
        user_id: str = Header(...),
        case_id: str = params.case_id,
        payload=api_integration.global_depends(),
    ):
        await api_integration.user_case_hook(user_id=user_id, case_id=case_id, auth_payload=payload)
        return await cds_connector.fetch_slides(case_id=case_id, skip=skip, limit=limit)
