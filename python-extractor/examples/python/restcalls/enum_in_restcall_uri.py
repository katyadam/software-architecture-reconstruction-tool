from typing import Tuple

from fastapi.exceptions import HTTPException

from ....singletons import http_client, idms_url
from ..custom_models.id_mapper import MappingType


async def fetch_mapping(empaia_id: str, mapping_type: MappingType) -> Tuple[str, str]:
    if idms_url is "":
        return None, None
    try:
        url = f"{idms_url}/v1/{mapping_type.value}/empaia/{empaia_id}"
        raw_mapping = await http_client.get(url)
        mds_url = raw_mapping["mds_url"] if "mds_url" in raw_mapping else None
        local_id = raw_mapping["local_id"] if "local_id" in raw_mapping else None
        return mds_url, local_id
    except HTTPException as e:
        if e.status_code == 404:
            return None, None
        else:
            raise e


async def fetch_mappings(empaia_ids: list, mapping_type: MappingType) -> dict:
    if idms_url is "":
        return None, None
    try:
        json = {"empaia_ids": empaia_ids}
        url = f"{idms_url}/v1/{mapping_type.value}/empaia/query"
        raw_mappings = await http_client.put(url, json=json)

        mappings = {}
        for mapping in raw_mappings["items"]:
            mappings[mapping["empaia_id"]] = mapping
        return mappings
    except HTTPException as e:
        if e.status_code == 404:
            return None, None
        else:
            raise e
