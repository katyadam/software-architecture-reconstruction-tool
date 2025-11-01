use models::{CallStatement, Callable, Import};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostIMCG {
    #[schema(example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    pub codebase_uuid: Uuid,

    #[schema(example = json!([
        {
            "signature": "unique signature of the callable",
            "namespace": "namespace where the callable lives",
            "parameters": [
                "skip", "limit", "q"
            ],
            "return_type": "int",
            "is_async": true,
            "is_constructor": false,
            "hash": "qwertzasdf"
        }
    ]))]
    pub callables: Vec<Callable>,

    #[schema(example = json!([
        {
            "function_name": "get_items",
            "arguments": [
                {
                    "assigned_variable": "params",
                    "value": "{\"skip\": skip, \"limit\": limit}"
                }
            ],
            "enclosing_function_name": "main",
            "enclosing_class_name": "class",
        }
    ]))]
    pub call_statements: Vec<CallStatement>,

    #[schema(example = json!([
        {
            "orig_module": "os.path",
            "orig_name": "join",
            "module_alias": "os_path",
            "name_alias": "join_path",
            "codeword": "import_statement_1"
        }
    ]))]
    pub imports: Vec<Import>,
}
