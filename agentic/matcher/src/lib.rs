//! Static matcher: REST calls -> endpoints into `Matches`, no model.
//! Every candidate becomes a link, as the synthesizer baseline does.

mod candidates;
mod group;
mod metrics;
mod points;

use std::collections::BTreeMap;

use agentic_models::{CodeElement, Matches, Sdg, Service};
use models::configuration::{ServiceDescription, assign_service};
use models::substitute::Substituter;
use models::{ConfigurationData, Endpoint, RestCall};

use candidates::candidates;
pub use metrics::Metrics;
use points::Placed;

/// Matches `calls` to `endpoints`; `constants` are `(name, value)` pairs.
pub fn match_static(
    endpoints: &[Endpoint],
    calls: &[RestCall],
    config: &ConfigurationData,
    constants: &[(String, String)],
) -> Result<(Matches, Metrics), regex::Error> {
    let services = &config.service_descriptions;
    let all_urls: Vec<String> = services.iter().flat_map(|s| s.urls.clone()).collect();
    let substituter = Substituter::new(constants)?;
    let mut metrics = Metrics::default();

    // Unassigned endpoints stay candidates under the empty service, as in the
    // baseline: they can still dominate or win fuzzy matching.
    let receivers: Vec<Placed> = endpoints
        .iter()
        .map(|e| {
            let sd = assign_or_log(&e.file_path, services, &mut metrics.unassigned_receivers);
            Placed::receiver(e.clone(), sd.cloned().unwrap_or_default())
        })
        .collect();
    let initiators: Vec<Placed> = calls
        .iter()
        .filter_map(|c| {
            let sd = assign_or_log(&c.file_path, services, &mut metrics.unassigned_initiators)?;
            Some(Placed::initiator(
                c.clone(),
                sd.clone(),
                substituter.apply(&c.target_uri),
            ))
        })
        .collect();
    metrics.receivers = receivers.len();

    let mut links = Vec::new();
    let mut unmatched = Vec::new();
    for call in &initiators {
        let found = candidates(call, &receivers, &all_urls);
        metrics.record(&found);
        if found.is_empty() {
            unmatched.push(group::no_candidate(call));
        }
        for (receiver, _) in found {
            links.push((
                receiver.point.service.clone(),
                group::link(call, receiver, &all_urls),
            ));
        }
    }

    let dependencies = group::dependencies(links);
    metrics.dependencies = dependencies.len();
    let sdg = Sdg {
        services: sdg_services(services, &receivers),
        dependencies,
    };
    let matches = Matches {
        sdg,
        unmatched,
        excluded: vec![],
    };
    Ok((matches, metrics))
}

/// The file's service; when there is none, logs it and bumps `unassigned`.
fn assign_or_log<'s>(
    file: &str,
    services: &'s [ServiceDescription],
    unassigned: &mut usize,
) -> Option<&'s ServiceDescription> {
    let found = assign_service(file, services);
    if found.is_none() {
        eprintln!("[matcher] unassigned: {file}");
        *unassigned += 1;
    }
    found
}

/// One service per name with its endpoints, sorted by name. Like the
/// baseline, the last config entry of a name wins and receivers' services are
/// added when no config entry has them.
fn sdg_services(services: &[ServiceDescription], receivers: &[Placed]) -> Vec<Service> {
    let service = |sd: &ServiceDescription| Service {
        name: sd.name.clone(),
        urls: sd.urls.clone(),
        endpoints: vec![],
    };
    let mut by_name: BTreeMap<&str, Service> = services
        .iter()
        .map(|sd| (sd.name.as_str(), service(sd)))
        .collect();
    for r in receivers {
        by_name
            .entry(&r.service.name)
            .or_insert_with(|| service(&r.service));
    }
    for r in receivers {
        if let (Some(s), CodeElement::Endpoint(e)) =
            (by_name.get_mut(r.service.name.as_str()), &r.point.element)
        {
            s.endpoints.push(e.clone());
        }
    }
    by_name.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svc(name: &str, base: &str) -> ServiceDescription {
        ServiceDescription {
            name: name.into(),
            base_dir_path: base.into(),
            urls: vec![format!("http://{name}:8000")],
            java_analysis: None,
        }
    }

    fn ep(file: &str) -> Endpoint {
        Endpoint {
            uri: "/x".into(),
            file_path: file.into(),
            ..Default::default()
        }
    }

    #[test]
    fn duplicate_names_merge_and_unassigned_kept() {
        let config = ConfigurationData {
            service_descriptions: vec![svc("mds", "p/mds1"), svc("mds", "p/mds2")],
        };
        let eps = [ep("p/mds1/a.py"), ep("p/mds2/b.py"), ep("p/tools/c.py")];
        let (m, metrics) = match_static(&eps, &[], &config, &[]).expect("matches");
        let got: Vec<_> = m
            .sdg
            .services
            .iter()
            .map(|s| (s.name.as_str(), s.endpoints.len()))
            .collect();
        assert_eq!(got, vec![("", 1), ("mds", 2)]);
        assert_eq!((metrics.receivers, metrics.unassigned_receivers), (3, 1));
    }
}
