use std::collections::HashMap;

use crate::{
    contextmap::model::{AssignedEntity, ContextMap, Dependency},
    errors::builder::BuilderError,
    utils::assign_service_description_to_file,
};

use models::{Entity, configuration::ServiceDescription};
pub trait ContextMapBuilder {
    fn build(
        &self,
        entities: &[Entity],
        service_descs: &[ServiceDescription],
    ) -> Result<ContextMap, BuilderError>;
}

pub struct ContextMapBuilderImpl {}

impl ContextMapBuilder for ContextMapBuilderImpl {
    fn build(
        &self,
        entities: &[Entity],
        service_descs: &[ServiceDescription],
    ) -> Result<ContextMap, BuilderError> {
        let collected_dependencies = self.connect_entities(entities, service_descs);
        let assigned_entities = self.get_assigned_entities(entities, service_descs);
        Ok(ContextMap {
            entities: assigned_entities,
            dependencies: collected_dependencies,
        })
    }
}

impl Default for ContextMapBuilderImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextMapBuilderImpl {
    pub fn new() -> Self {
        Self {}
    }

    fn get_assigned_entities(
        &self,
        entities: &[Entity],
        service_descs: &[ServiceDescription],
    ) -> Vec<AssignedEntity> {
        entities
            .iter()
            .map(|entity| {
                let service_desc =
                    assign_service_description_to_file(&entity.file_path, service_descs);
                AssignedEntity::new(entity.clone(), service_desc.name)
            })
            .collect()
    }

    fn connect_entities(
        &self,
        entities: &[Entity],
        service_descs: &[ServiceDescription],
    ) -> Vec<Dependency> {
        // 1. Map Entity Signatures to Service Names
        let mut ms_names: HashMap<String, String> = HashMap::new();
        for entity in entities {
            let service_desc =
                crate::utils::assign_service_description_to_file(&entity.file_path, service_descs);
            ms_names.insert(entity.signature.clone(), service_desc.name);
        }

        // 2. Identify directed multiplicities
        // Key: (SourceSig, TargetSig), Value: (MultiplicityCount, (SourceMS, TargetMS))
        // We use -1 to represent "Many" (0..*)
        let mut mults: HashMap<(String, String), (i32, (String, String))> = HashMap::new();

        for entity in entities {
            let current_ms = ms_names.get(&entity.signature).cloned().unwrap_or_default();

            for field in &entity.fields {
                // If we resolved a signature, check if it points to a known Entity
                if let Some(target_sig) = &field.datatype_signature
                    && ms_names.contains_key(target_sig)
                {
                    let key = (entity.signature.clone(), target_sig.clone());
                    let target_ms = ms_names.get(target_sig).cloned().unwrap_or_default();

                    // Determine multiplicity: if it's a collection, it's -1 (0..*)
                    let current_mult = if field.is_collection { -1 } else { 1 };

                    mults
                        .entry(key)
                        .and_modify(|(count, _)| {
                            // If it's already a collection or we just found a collection, keep it -1
                            if *count != -1 && current_mult != -1 {
                                *count += 1;
                            } else if current_mult == -1 {
                                *count = -1;
                            }
                        })
                        .or_insert((current_mult, (current_ms.clone(), target_ms)));
                }
            }
        }

        // 3. Combine into bidirectional links
        // We group A->B and B->A into a single entry with two multiplicities
        let mut links: HashMap<(String, String), ((i32, i32), (String, String))> = HashMap::new();

        for ((src, tgt), (mult, (ms_src, ms_tgt))) in mults {
            if let Some(val) = links.get_mut(&(tgt.clone(), src.clone())) {
                // We found the reverse link (B->A), so update the second multiplicity
                val.0.1 = mult;
            } else {
                // New link or forward link (A->B)
                links
                    .entry((src.clone(), tgt.clone()))
                    .and_modify(|val| {
                        // This handles multiple fields of same type between same entities
                        if val.0.0 != -1 {
                            if mult == -1 {
                                val.0.0 = -1;
                            } else {
                                val.0.0 += mult;
                            }
                        }
                    })
                    .or_insert(((mult, 0), (ms_src, ms_tgt)));
            }
        }

        // 4. Final conversion to DependencyExtended objects
        links
            .into_iter()
            .map(|((src, tgt), ((m1, m2), (ms1, ms2)))| {
                let format_mult = |m: i32| {
                    if m == -1 {
                        "0..*".to_string()
                    } else {
                        m.to_string()
                    }
                };
                Dependency {
                    source_id: src,
                    target_id: tgt,
                    source_multiplier: format_mult(m1),
                    target_multiplier: format_mult(m2),
                    source_ms: ms1,
                    target_ms: ms2,
                }
            })
            .collect()
    }
}
