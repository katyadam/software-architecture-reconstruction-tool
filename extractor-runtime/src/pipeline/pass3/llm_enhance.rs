use std::collections::HashMap;

use models::{
    ConfigurationData,
    assignments::{AssignmentKey, Scope, VariableAddress},
    ir::project::ProjectIR,
};

use crate::pipeline::pass3::{pass_attr::PerFileAttrMap, pass_module::PerFileModuleConsts};

pub fn build_variable_map(
    config: &ConfigurationData,
    project_ir: &ProjectIR,
    per_file_attrs: &PerFileAttrMap,
    per_file_module_attrs: &PerFileModuleConsts,
) -> HashMap<VariableAddress, String> {
    let mut map = HashMap::new();

    for (name, constant) in &project_ir.constants {
        let addr = VariableAddress {
            microservice: microservice_for_file(&constant.source_file, config),
            file: constant.source_file.clone(),
            key: AssignmentKey {
                scope: Scope::Global,
                variable_name: name.clone(),
            },
        };
        map.insert(addr, constant.value.clone());
    }

    for file in &project_ir.files {
        let microservice = microservice_for_file(&file.file_path, config);
        for (key, assignment) in &file.assignments {
            if matches!(key.scope, Scope::Function(_)) {
                continue;
            }
            let addr = VariableAddress {
                microservice: microservice.clone(),
                file: file.file_path.clone(),
                key: key.clone(),
            };
            map.insert(addr, assignment.value.clone());
        }
    }

    for (file_path, attr_map) in per_file_attrs {
        let microservice = microservice_for_file(file_path, config);
        for (dotted_key, value) in attr_map {
            let addr = VariableAddress {
                microservice: microservice.clone(),
                file: file_path.clone(),
                key: AssignmentKey {
                    scope: Scope::Global,
                    variable_name: dotted_key.clone(),
                },
            };
            map.insert(addr, value.clone());
        }
    }

    for (file_path, module_consts) in per_file_module_attrs {
        let microservice = microservice_for_file(file_path, config);
        for (var_name, value) in module_consts {
            let addr = VariableAddress {
                microservice: microservice.clone(),
                file: file_path.clone(),
                key: AssignmentKey {
                    scope: Scope::Global,
                    variable_name: var_name.clone(),
                },
            };
            map.insert(addr, value.clone());
        }
    }

    map
}

fn microservice_for_file(file_path: &str, config: &ConfigurationData) -> String {
    config
        .service_descriptions
        .iter()
        .find(|s| file_path.contains(&s.base_dir_path))
        .map(|s| s.name.clone())
        .unwrap_or_default()
}
