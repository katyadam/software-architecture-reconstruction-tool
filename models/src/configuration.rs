use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConfigurationData {
    pub service_descriptions: Vec<ServiceDescription>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Default, Clone)]
pub struct ServiceDescription {
    pub name: String,
    pub base_dir_path: String,
    pub urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_analysis: Option<JavaAnalysisSettings>,
}

/// Stable build-input settings; resolved paths and dependencies live in a build manifest.
#[derive(Debug, Serialize, Deserialize, ToSchema, Default, Clone)]
pub struct JavaAnalysisSettings {
    #[serde(default)]
    pub maven_profiles: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifacts_file: Option<String>,
}

/// The service whose base directory holds `file`; `None` when no service does.
pub fn assign_service<'s>(
    file: &str,
    services: &'s [ServiceDescription],
) -> Option<&'s ServiceDescription> {
    let file = file.replace('\\', "/");
    services.iter().find(|sd| {
        let base = sd.base_dir_path.replace('\\', "/");
        file.starts_with(&base) || file.ends_with(&base) || file.contains(&format!("/{base}/"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svc(name: &str, base: &str) -> ServiceDescription {
        ServiceDescription {
            name: name.into(),
            base_dir_path: base.into(),
            ..Default::default()
        }
    }

    #[test]
    fn assigned_by_base_dir() {
        let services = [
            svc("app-service", "empaia/app-service"),
            svc("wbs", "empaia/workbench-service"),
        ];
        let got = assign_service("..\\empaia\\workbench-service\\wbs\\client.py", &services);
        assert_eq!(got.map(|s| s.name.as_str()), Some("wbs"));
        assert!(assign_service("../empaia/tools/x.py", &services).is_none());
    }
}
