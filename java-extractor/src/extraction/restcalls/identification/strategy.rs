use models::HttpMethod;

pub trait Strategy<'a> {
    fn identify_http_method(&self) -> Option<HttpMethod>;
    fn identify_target_uri(&self) -> Option<String>;
}
