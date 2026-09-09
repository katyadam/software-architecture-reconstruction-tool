use python_extractor::extraction::parse::extract_syntactic;

#[test]
fn identifies_aio_stub_operations() {
    let code = r#"
class DocumentServiceClient:
    def __init__(self):
        self.stub = document_service_pb2_grpc.DocumentServiceStub(channel)

    async def get(self):
        return await self.stub.GetDocument(request)
"#;
    let record = extract_syntactic(code, "grpc_client.py").unwrap();
    assert_eq!(record.raw_restcalls.len(), 1);
    assert_eq!(
        record.raw_restcalls[0].target_uri,
        "grpc://DocumentService/GetDocument"
    );
}

#[test]
fn identifies_module_level_stub_operations() {
    let code = r#"
from todo_pb2_grpc import TodoServiceStub

stub = TodoServiceStub(channel)

def list_todos():
    return stub.List(request)
"#;
    let record = extract_syntactic(code, "grpc_client.py").unwrap();
    assert_eq!(record.raw_restcalls.len(), 1);
    assert_eq!(
        record.raw_restcalls[0].target_uri,
        "grpc://TodoService/List"
    );
    assert_eq!(record.raw_restcalls[0].function_name, "list_todos() -> Any");
}

#[test]
fn identifies_servicer_operations() {
    let code = r#"
class DocumentServicer(document_service_pb2_grpc.DocumentServiceServicer):
    async def GetDocument(self, request, context):
        return response
"#;
    let record = extract_syntactic(code, "grpc_servicer.py").unwrap();
    assert_eq!(record.endpoints.len(), 1);
    assert_eq!(
        record.endpoints[0].uri,
        "grpc://DocumentService/GetDocument"
    );
}

#[test]
fn identifies_synchronous_servicer_operations() {
    let code = r#"
class TodoServicer(todo_pb2_grpc.TodoServiceServicer):
    def List(self, request, context):
        return response
"#;
    let record = extract_syntactic(code, "grpc_servicer.py").unwrap();
    assert_eq!(record.endpoints.len(), 1);
    assert_eq!(record.endpoints[0].uri, "grpc://TodoService/List");
}
