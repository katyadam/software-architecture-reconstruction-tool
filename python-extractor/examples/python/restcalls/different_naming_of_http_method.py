from fastapi import Request

from ....models.v1.annotation.collections import (
    Collection,
    CollectionList,
    CollectionQuery,
    ItemPostResponse,
    ItemQuery,
    ItemQueryList,
    PostCollection,
    PostItems,
)
from ....models.v1.commons import Id, IdObject, Message, UniqueReferences
from ....singletons import api_integration, http_client, settings
from ..helpers import params


def add_routes_collections(app):
    base_url = settings.as_url.rstrip("/")

    @app.get(
        "/collections",
        tags=["Annotation"],
        responses={
            200: {"model": CollectionList},
        },
    )
    async def _(skip: int = None, limit: int = None, _=api_integration.global_depends()):
        return await http_client.get_stream_response(
            f"{base_url}/v1/collections", params=params(skip=skip, limit=limit)
        )

    @app.post(
        "/collections",
        tags=["Annotation"],
        status_code=201,
        responses={201: {"model": Collection}},
    )
    async def _(
        collection: PostCollection, request: Request, external_ids: bool = False, _=api_integration.global_depends()
    ):
        return await http_client.post_stream_response(
            f"{base_url}/v1/collections",
            data=request.stream(),
            params=params(external_ids=external_ids),
            headers={"Content-Type": "application/json"},
        )

    @app.put(
        "/collections/query",
        tags=["Annotation"],
        status_code=200,
        responses={200: {"model": CollectionList}},
    )
    async def _(
        collections_query: CollectionQuery,
        skip: int = None,
        limit: int = None,
        _=api_integration.global_depends(),
    ):
        return await http_client.put_stream_response(
            f"{base_url}/v1/collections/query",
            json=collections_query.model_dump(),
            params=params(skip=skip, limit=limit),
        )

    @app.put(
        "/collections/query/unique-references",
        tags=["Annotation"],
        status_code=200,
        responses={
            200: {"model": UniqueReferences},
        },
    )
    async def _(
        collections_query: CollectionQuery,
        _=api_integration.global_depends(),
    ):
        return await http_client.put_stream_response(
            f"{base_url}/v1/collections/query/unique-references",
            json=collections_query.model_dump(),
        )

    @app.get(
        "/collections/{collection_id}",
        tags=["Annotation"],
        responses={
            200: {"model": Collection},
            404: {
                "model": Message,
                "description": "The resource was not found",
            },
        },
    )
    async def _(
        collection_id: Id,
        shallow: bool = False,
        with_leaf_ids: bool = False,
        _=api_integration.global_depends(),
    ):
        return await http_client.get_stream_response(
            f"{base_url}/v1/collections/{collection_id}", params=params(shallow=shallow, with_leaf_ids=with_leaf_ids)
        )

    @app.put(
        "/collections/{collection_id}",
        tags=["Annotation"],
        responses={
            200: {"model": Collection},
            404: {
                "model": Message,
                "description": "The resource was not found",
            },
            423: {"model": Message, "description": "Collection is locked"},
        },
    )
    async def _(collection_id: Id, collection: Collection, _=api_integration.global_depends()):
        return await http_client.put_stream_response(
            f"{base_url}/v1/collections/{collection_id}", json=collection.model_dump()
        )

    @app.delete(
        "/collections/{collection_id}",
        tags=["Annotation"],
        responses={
            200: {"model": IdObject},
            404: {
                "model": Message,
                "description": "The resource was not found",
            },
            423: {"model": Message, "description": "Collection is locked"},
        },
    )
    async def _(collection_id: Id, _=api_integration.global_depends()):
        return await http_client.delete_stream_response(f"{base_url}/v1/collections/{collection_id}")

    @app.post(
        "/collections/{collection_id}/items",
        tags=["Annotation"],
        status_code=201,
        responses={
            201: {"model": ItemPostResponse},
            404: {
                "model": Message,
                "description": "The resource was not found",
            },
            405: {"model": Message, "description": "Resource is locked"},
        },
    )
    async def _(
        collection_id: Id,
        items: PostItems,
        request: Request,
        external_ids: bool = False,
        _=api_integration.global_depends(),
    ):
        return await http_client.post_stream_response(
            f"{base_url}/v1/collections/{collection_id}/items",
            data=request.stream(),
            params=params(external_ids=external_ids),
            headers={"Content-Type": "application/json"},
        )

    @app.put(
        "/collections/{collection_id}/items/query",
        tags=["Annotation"],
        status_code=200,
        responses={200: {"model": ItemQueryList}},
    )
    async def _(
        collection_id: Id,
        items_query: ItemQuery,
        skip: int = None,
        limit: int = None,
        _=api_integration.global_depends(),
    ):
        return await http_client.put_stream_response(
            f"{base_url}/v1/collections/{collection_id}/items/query",
            json=items_query.model_dump(),
            params=params(skip=skip, limit=limit),
        )

    @app.put(
        "/collections/{collection_id}/items/query/unique-references",
        tags=["Annotation"],
        status_code=200,
        responses={
            200: {"model": UniqueReferences},
        },
    )
    async def _(
        collection_id: Id,
        items_query: ItemQuery,
        _=api_integration.global_depends(),
    ):
        return await http_client.put_stream_response(
            f"{base_url}/v1/collections/{collection_id}/items/query/unique-references",
            json=items_query.model_dump(),
        )

    @app.delete(
        "/collections/{collection_id}/items/{item_id}",
        tags=["Annotation"],
        responses={
            200: {"model": IdObject},
            404: {
                "model": Message,
                "description": "The resource was not found",
            },
            405: {"model": Message, "description": "Resource is locked"},
        },
    )
    async def _(collection_id: Id, item_id: Id, _=api_integration.global_depends()):
        return await http_client.delete_stream_response(f"{base_url}/v1/collections/{collection_id}/items/{item_id}")
