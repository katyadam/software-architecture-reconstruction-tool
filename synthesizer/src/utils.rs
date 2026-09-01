use models::configuration::ServiceDescription;

pub fn assign_service_description_to_file(
    file_name: &str,
    service_descs: &[ServiceDescription],
) -> ServiceDescription {
    service_descs
        .iter()
        // Files collected by the CLI may be absolute while configurations are
        // normally repository-relative.  Accept the latter as a path segment so
        // portable benchmark configurations work from a separate worktree too.
        .find(|sd| {
            file_name.starts_with(&sd.base_dir_path)
                || file_name.contains(&format!("/{}", sd.base_dir_path.trim_matches('/')))
        })
        .cloned()
        .unwrap_or_default()
}
