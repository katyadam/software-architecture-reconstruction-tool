use models::configuration::ServiceDescription;

pub fn assign_service_description_to_file(
    file_name: &str,
    service_descs: &[ServiceDescription],
) -> ServiceDescription {
    service_descs
        .iter()
        .find(|sd| file_name.starts_with(&sd.base_dir_path))
        .cloned()
        .unwrap_or_default()
}
