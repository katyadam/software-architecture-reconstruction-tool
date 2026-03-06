def add_routes_classes(app):
    base_url = settings.as_url.rstrip("/")

    @app.get(
        "/classes",
        tags=["Annotation"],
        responses={
            200: {"model": ClassList},
        },
    )
    async def _(
        with_unique_class_values: bool = False,
        skip: int = None,
        limit: int = None,
        case_id: Annotated[str | None, Header()] = None,
        payload=api_integration.global_depends(),
    ):
        await api_integration.case_hook(case_id=case_id, auth_payload=payload)
        return await http_client.get_stream_response(
            f"{base_url}/v3/classes",
            params={"with_unique_class_values": with_unique_class_values,
                    "skip": skip, "limit": limit},
            headers={"case-id": case_id},
        )

    @app.post(
        "/classes",
        tags=["Annotation"],
        status_code=201,
        responses={201: {"model": Classes}},
    )
    async def _(
        classes: PostClasses,
        request: Request,
        external_ids: bool = False,
        case_id: Annotated[str | None, Header()] = None,
        payload=api_integration.global_depends(),
    ):
        await api_integration.case_hook(case_id=case_id, auth_payload=payload)
        return await http_client.post_stream_response(
            f"{base_url}/v3/classes",
            data=request.stream(),
            params={"external_ids": external_ids},
            headers={"case-id": case_id, "Content-Type": "application/json"},
        )

    @app.put(
        "/classes/query",
        tags=["Annotation"],
        status_code=200,
        responses={200: {"model": ClassList}},
    )
    async def _(
        classes_query: ClassQuery,
        with_unique_class_values: bool = False,
        skip: int = None,
        limit: int = None,
        case_id: Annotated[str | None, Header()] = None,
        payload=api_integration.global_depends(),
    ):
        await api_integration.case_hook(case_id=case_id, auth_payload=payload)
        return await http_client.put_stream_response(
            f"{base_url}/v3/classes/query",
            json=classes_query.model_dump(),
            params={"with_unique_class_values": with_unique_class_values,
                    "skip": skip, "limit": limit},
            headers={"case-id": case_id},
        )

    @app.put(
        "/classes/query/unique-references",
        tags=["Annotation"],
        status_code=200,
        responses={
            200: {"model": UniqueReferences},
        },
    )
    async def _(
        classes_query: ClassQuery,
        case_id: Annotated[str | None, Header()] = None,
        payload=api_integration.global_depends(),
    ):
        await api_integration.case_hook(case_id=case_id, auth_payload=payload)
        return await http_client.put_stream_response(
            f"{base_url}/v3/classes/query/unique-references",
            json=classes_query.model_dump(),
            headers={"case-id": case_id},
        )

    @app.get(
        "/classes/{class_id}",
        tags=["Annotation"],
        responses={
            200: {"model": Class},
            404: {
                "model": Message,
                "description": "The resource was not found",
            },
        },
    )
    async def _(
        class_id: Id, case_id: Annotated[str | None, Header()] = None, payload=api_integration.global_depends()
    ):
        await api_integration.case_hook(case_id=case_id, auth_payload=payload)
        return await http_client.get_stream_response(
            f"{base_url}/v3/classes/{class_id}",
            headers={"case-id": case_id},
        )

    @app.put(
        "/classes/{class_id}",
        tags=["Annotation"],
        responses={
            200: {"model": Class},
            404: {
                "model": Message,
                "description": "The resource was not found",
            },
            423: {"model": Message, "description": "Class is locked"},
        },
    )
    async def _(
        class_id: Id,
        put_class: PostClass,
        request: Request,
        case_id: Annotated[str | None, Header()] = None,
        payload=api_integration.global_depends(),
    ):
        await api_integration.case_hook(case_id=case_id, auth_payload=payload)
        return await http_client.put_stream_response(
            f"{base_url}/v3/classes/{class_id}",
            data=request.stream(),
            headers={"case-id": case_id, "Content-Type": "application/json"},
        )

    @app.delete(
        "/classes/{class_id}",
        tags=["Annotation"],
        responses={
            200: {"model": IdObject},
            404: {
                "model": Message,
                "description": "The resource was not found",
            },
            423: {"model": Message, "description": "Class is locked"},
        },
    )
    async def _(
        class_id: Id, case_id: Annotated[str | None, Header()] = None, payload=api_integration.global_depends()
    ):
        await api_integration.case_hook(case_id=case_id, auth_payload=payload)
        return await http_client.delete_stream_response(
            f"{base_url}/v3/classes/{class_id}",
            headers={"case-id": case_id},
        )
