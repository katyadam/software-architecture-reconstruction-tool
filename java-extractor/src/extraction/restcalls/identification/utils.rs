use models::Argument;

pub fn http_method_in_call_arguments(call_args: &[Argument], http_method: &str) -> bool {
    call_args.iter().any(|arg| {
        arg.value.to_ascii_lowercase().contains(http_method)
            || arg.datatype.to_ascii_lowercase().contains(http_method)
    })
}
