use models::configuration::ServiceDescription;

pub fn assign_service_description_to_file(
    file_name: &str,
    service_descs: &[ServiceDescription],
) -> ServiceDescription {
    let normalized_file = file_name.replace('\\', "/");
    service_descs
        .iter()
        .find(|sd| {
            let normalized_base = sd.base_dir_path.replace('\\', "/");
            normalized_file.starts_with(&normalized_base)
                || normalized_file.ends_with(&normalized_base)
                || normalized_file.contains(&format!("/{normalized_base}/"))
        })
        .cloned()
        .unwrap_or_default()
}
