pub fn extract_invoked_object(call: &str) -> Option<&str> {
    let callee = call.split_once('(')?.0;
    let dot_pos = callee.rfind('.')?;
    Some(&callee[..dot_pos])
}
