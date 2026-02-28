const HTTP_METHODS: &[&str] = &[
    "get",
    "post",
    "delete",
    "put",
    "patch",
    "get_stream_response",
    "post_stream_response",
    "delete_stream_response",
    "put_stream_response",
    "patch_stream_response",
];
pub mod method_call;
pub mod strategy;
