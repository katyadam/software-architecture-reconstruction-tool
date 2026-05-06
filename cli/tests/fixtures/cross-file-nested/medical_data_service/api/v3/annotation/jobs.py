from ....singletons import settings


def add_routes_jobs(app):
    base_url = settings.as_url.rstrip("/")

    @app.put("/jobs/{job_id}/lock/annotations/{annotation_id}")
    async def _(job_id, annotation_id):
        return await http_client.put_stream_response(
            f"{base_url}/v3/jobs/{job_id}/lock/annotations/{annotation_id}"
        )
