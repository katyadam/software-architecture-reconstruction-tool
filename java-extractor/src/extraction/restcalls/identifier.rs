use models::Argument;

pub fn is_restcall(callable_name: String, call_args: Vec<Argument>) -> bool {
    if callable_name == "exchange" {
        return true;
    }

    false
}
