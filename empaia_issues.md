- This is in a test where medical-data-service calls itself in test, but the match ends up with workbench-service endpoint
- It is due to settings.mds_url being knows - can be added to constants

```json
{
    "endpoint": {
        "function_name": "_",
        "function_hash": "3e6a5c0291eb81eac21127ff9fa4a8539e41c21e5a79eb739c8cbeef83eb3dfa",
        "http_method": "GET",
        "parameters": [],
        "uri": "/alive",
        "file_path": "empaia/workbench-service/workbench_service/app.py"
    },
    "restcall": {
        "function_name": "wait_for_services() -> Any",
        "function_hash": "ffd0caa6f1a99da899cdf765862bf900c6211959e181dd95466ecd6ea0b83501",
        "call_arguments": [
            {
                "assigned_variable": "",
                "value": "settings.mds_url + \"/alive\"",
                "datatype": "any"
            }
        ],
        "http_method": "GET",
        "target_uri": "/alive",
        "file_path": "empaia/medical-data-service/tests/non_auth/v3/test_endpoints.py"
    }
}
```

- This is a test in annotation service that calls its own endpoint.
- Issue is that base_url cant be resolved to value, therefore the best match is workbench_service which is wrong
- Since base_url constant is used elsewhere, it cant be put into constants list.
- The easiest option is to omit processing test files.

```json
"endpoint": {
    "function_name": "_",
    "function_hash": "c05f81c2def25070a7331dca541fefede1e1bad5d32377d7a11f276a768797c9",
    "http_method": "PUT",
    "parameters": [
        {
            "name": "query",
            "datatype": "CollectionQuery",
            "initial_value": null
        },
        {
            "name": "skip",
            "datatype": "int",
            "initial_value": "None"
        },
        {
            "name": "limit",
            "datatype": "int",
            "initial_value": "None"
        },
        {
            "name": "scope_id",
            "datatype": "Id",
            "initial_value": "Path(...)"
        },
        {
            "name": "payload",
            "datatype": null,
            "initial_value": "scope_validation.scope_depends()"
        }
    ],
    "uri": "/{scope_id}/collections/query",
    "file_path": "empaia/workbench-service/workbench_service/api/v3/routes/scopes/data.py"
},
"restcall": {
    "function_name": "query_over_limit(base_url, up) -> Any",
    "function_hash": "044fd98e207392872302e6bf22078314f5c8fcf4edbc9aefe8898f9b233be3b3",
    "call_arguments": [
        {
            "assigned_variable": "",
            "value": "f\"{base_url}/collections/query\"",
            "datatype": "any"
        },
        {
            "assigned_variable": "json",
            "value": "query_one",
            "datatype": "any"
        }
    ],
    "http_method": "PUT",
    "target_uri": "{base_url}/collections/query",
    "file_path": "empaia/annotation-service/tests/v1/test_04_collections/collections_query.py"
}
```