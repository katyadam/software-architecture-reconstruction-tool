use models::{HttpMethod, RestCall};

mod go;
mod java;
mod python;

/// Minimal Java RestCall for pass 3 testing.
///
/// Java `extract_syntactic` does not run `evaluate_invocations`, so `invoked_on`
/// is never set and `SpringIdentificationStrategy` produces no raw_restcalls.
/// Tests add them manually instead.
fn java_restcall(function_name: &str, target_uri: &str, file_path: &str) -> RestCall {
    RestCall {
        function_name: function_name.to_string(),
        function_hash: String::new(),
        call_arguments: vec![],
        http_method: HttpMethod::GET,
        target_uri: target_uri.to_string(),
        file_path: file_path.to_string(),
    }
}
