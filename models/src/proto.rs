/// A gRPC service contract declared in a protobuf file.
#[derive(Debug, Clone)]
pub struct ProtoService {
    pub name: String,
    pub operations: Vec<String>,
    pub file_path: String,
}
