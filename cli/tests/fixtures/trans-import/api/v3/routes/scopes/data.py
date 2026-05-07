# pylint: disable=function-redefined

from typing import Annotated, Dict
from ...singletons import api_integration, http_client, scope_validation, settings


def add_routes(app, late_init):
    # ANNOTATIONS

    @app.put(
        "/{scope_id}/pixelmaps/{pixelmap_id}/level/{level}/position/start/{start_x}/{start_y}/end/{end_x}/{end_y}/data",
        tags=["data"],
        summary="Bulk tile upload for a pixelmap",
        status_code=204,
        responses={
            204: {},
            404: {"model": Message, "description": "Pixelmap not found"},
            413: {"model": Message, "description": "Requested data too large"},
            422: {"model": Message, "description": "Invalid parameters"},
            423: {"model": Message, "description": "Pixelmap is locked"},
        },
    )
    async def _(
        request: Request,
        scope_id: Id = Path(...),
        pixelmap_id: Id = params.pixelmap_id,
        level: int = Path(
            ge=0, example=0, description="Pyramid level of region"),
        start_x: int = Path(
            example=0, description="Start position in x dimension"),
        start_y: int = Path(
            example=0, description="Start position in y dimension"),
        end_x: int = Path(
            example=0, description="End position in x dimension"),
        end_y: int = Path(
            example=0, description="End position in y dimension"),
        content_encoding: Annotated[str | None, Header()] = None,
        payload=scope_validation.scope_depends(),
    ):
        await api_integration.scope_hook(scope_id, payload)
        await es_connector.validate_examination_state_open(scope_id=scope_id)
        mds_url = settings.medical_data_service_url + "/v3/pixelmaps"
        headers = {
            "Content-Type": "application/octet-stream",
            "Content-Encoding": content_encoding,
            "case-id": await es_connector.get_case_id(scope_id),
        }
        return await http_client.put_stream_response(
            f"{mds_url}/{pixelmap_id}/level/{level}/position/start/{start_x}/{start_y}/end/{end_x}/{end_y}/data",
            data=request.stream(),
            headers=headers,
        )
