use models::ConfigurationData;

pub(super) fn microservice_for_file(file_path: &str, config: &ConfigurationData) -> String {
    config
        .service_descriptions
        .iter()
        .find(|s| file_path.contains(&s.base_dir_path))
        .map(|s| s.name.clone())
        .unwrap_or_default()
}
