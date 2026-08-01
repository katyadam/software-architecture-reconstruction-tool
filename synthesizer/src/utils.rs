use models::configuration::ServiceDescription;

pub fn assign_service_description_to_file(
    file_name: &str,
    service_descs: &[ServiceDescription],
) -> ServiceDescription {
    service_descs
        .iter()
        .find(|sd| {
            file_name.starts_with(&sd.base_dir_path)
                || file_name.contains(&format!("/{}", sd.base_dir_path.trim_matches('/')))
        })
        .cloned()
        .unwrap_or_default()
}
