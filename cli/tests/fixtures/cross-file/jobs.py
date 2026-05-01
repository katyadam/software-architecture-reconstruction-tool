from pydantic import UUID4

from ....models.v1.annotation.annotations import AnnotationList
from ....models.v1.annotation.collections import SlideList
from ....models.v1.annotation.jobs import JobLock, NodeType, PrimitiveDetails, TreeNodeSequence
from ....models.v1.annotation.primitives import PrimitiveList
from ....models.v1.commons import Id, Message
from singletons import api_integration, http_client, settings
from ..helpers import params


def add_routes_jobs(app):
    base_url = settings.as_url.rstrip("/")

    @app.put(
        "/jobs/{job_id}/lock/annotations/{annotation_id}",
        tags=["Annotation"],
        responses={
            200: {"model": JobLock},
            404: {
                "model": Message,
                "description": "The resource was not found",
            },
        },
    )
    async def _(job_id: Id, annotation_id: Id, _=api_integration.global_depends()):
        url = base_url
        return await http_client.put_stream_response(f"{url}/v1/jobs/{job_id}/lock/annotations/{annotation_id}")

    @app.put(
        "/jobs/{job_id}/lock/classes/{class_id}",
        tags=["Annotation"],
        responses={
            200: {"model": JobLock},
            404: {
                "model": Message,
                "description": "The resource was not found",
            },
        },
    )
    async def _(job_id: Id, class_id: Id, _=api_integration.global_depends()):
        return await http_client.put_stream_response(f"{base_url}/v1/jobs/{job_id}/lock/classes/{class_id}")

    @app.put(
        "/jobs/{job_id}/lock/collections/{collection_id}",
        tags=["Annotation"],
        responses={
            200: {"model": JobLock},
            404: {
                "model": Message,
                "description": "The resource was not found",
            },
        },
    )
    async def _(job_id: Id, collection_id: Id, _=api_integration.global_depends()):
        return await http_client.put_stream_response(f"{base_url}/v1/jobs/{job_id}/lock/collections/{collection_id}")

    @app.put(
        "/jobs/{job_id}/lock/primitives/{primitive_id}",
        tags=["Annotation"],
        responses={
            200: {"model": JobLock},
            404: {
                "model": Message,
                "description": "The resource was not found",
            },
        },
    )
    async def _(job_id: Id, primitive_id: Id, _=api_integration.global_depends()):
        return await http_client.put_stream_response(f"{base_url}/v1/jobs/{job_id}/lock/primitives/{primitive_id}")

    @app.put(
        "/jobs/{job_id}/lock/slides/{slide_id}",
        tags=["Annotation"],
        responses={
            200: {"model": JobLock},
            404: {
                "model": Message,
                "description": "The resource was not found",
            },
        },
    )
    async def _(job_id: Id, slide_id: Id, _=api_integration.global_depends()):
        return await http_client.put_stream_response(f"{base_url}/v1/jobs/{job_id}/lock/slides/{slide_id}")

    @app.get(
        "/jobs/{job_id}/tree/items",
        tags=["Deprecated"],
        responses={
            200: {"model": SlideList},
        },
    )
    async def _(job_id: Id, skip: int = None, limit: int = None, _=api_integration.global_depends()):
        return await http_client.get_stream_response(
            f"{base_url}/v1/jobs/{job_id}/tree/items", params=params(skip=skip, limit=limit)
        )

    @app.get(
        "/jobs/{job_id}/tree/primitives",
        tags=["Deprecated"],
        responses={
            200: {"model": PrimitiveList},
        },
    )
    async def _(job_id: Id, skip: int = None, limit: int = None, _=api_integration.global_depends()):
        return await http_client.get_stream_response(
            f"{base_url}/v1/jobs/{job_id}/tree/primitives", params=params(skip=skip, limit=limit)
        )

    @app.get(
        "/jobs/{job_id}/tree/primitives/{primitive_id}/details",
        tags=["Deprecated"],
        responses={
            200: {"model": PrimitiveDetails},
        },
    )
    async def _(job_id: Id, primitive_id: UUID4, _=api_integration.global_depends()):
        return await http_client.get_stream_response(
            f"{base_url}/v1/jobs/{job_id}/tree/primitives/{primitive_id}/details"
        )

    @app.get(
        "/jobs/{job_id}/tree/nodes/{node_type}/{node_id}/items",
        tags=["Deprecated"],
        responses={
            200: {"model": AnnotationList},
        },
    )
    async def _(
        job_id: Id,
        node_type: NodeType,
        node_id: Id,
        skip: int = None,
        limit: int = None,
        _=api_integration.global_depends(),
    ):
        return await http_client.get_stream_response(
            f"{base_url}/v1/jobs/{job_id}/tree/nodes/{node_type.value}/{node_id}/items",
            params=params(skip=skip, limit=limit),
        )

    @app.get(
        "/jobs/{job_id}/tree/nodes/{node_type}/{node_id}/primitives",
        tags=["Deprecated"],
        responses={
            200: {"model": PrimitiveList},
        },
    )
    async def _(
        job_id: Id,
        node_type: NodeType,
        node_id: Id,
        skip: int = None,
        limit: int = None,
        _=api_integration.global_depends(),
    ):
        return await http_client.get_stream_response(
            f"{base_url}/v1/jobs/{job_id}/tree/nodes/{node_type.value}/{node_id}/primitives",
            params=params(skip=skip, limit=limit),
        )

    @app.get(
        "/jobs/{job_id}/tree/items/{item_id}/sequence",
        tags=["Deprecated"],
        responses={
            200: {"model": TreeNodeSequence},
        },
    )
    async def _(
        job_id: Id,
        item_id: UUID4,
        _=api_integration.global_depends(),
    ):
        return await http_client.get_stream_response(
            f"{base_url}/v1/jobs/{job_id}/tree/items/{item_id}/sequence",
        )
