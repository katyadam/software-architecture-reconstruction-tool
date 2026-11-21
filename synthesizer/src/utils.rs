use models::configuration::{ServiceDescription, assign_service};

/// Default (empty) description when no service holds `file_name`.
pub fn assign_service_description_to_file(
    file_name: &str,
    service_descs: &[ServiceDescription],
) -> ServiceDescription {
    assign_service(file_name, service_descs)
        .cloned()
        .unwrap_or_default()
}
