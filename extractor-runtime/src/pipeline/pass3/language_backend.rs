use std::collections::HashMap;

use java_extractor::extraction::restcalls::evaluation::uri_generator::generate_target_uris as java_generate_target_uris;
use models::ir::language::Language;
use python_extractor::extraction::restcalls::evaluation::uri_generator::generate_target_uris as python_generate_target_uris;
use statix::{
    go::matcher::GoCallableMatcher, java::matcher::JavaCallableMatcher, matcher::CallableMatcher,
    python::matcher::PythonCallableMatcher, symbolic::AnalysisResult,
};

/// Language-specific behaviour needed during Pass 3 REST-call evaluation.
///
/// Implement this trait to add support for a new language:
/// 1. Create a zero-sized struct for the language.
/// 2. Implement `matcher` and `generate_uris`.
/// 3. Add one arm to `backend_for`.
pub(crate) trait LanguageSpecificEvaluator {
    fn matcher(&self) -> Box<dyn CallableMatcher>;
    fn generate_uris(
        &self,
        template: &str,
        analysis: &AnalysisResult,
        enums: &HashMap<String, Vec<String>>,
    ) -> Vec<String>;
}

struct JavaEvaluator;
struct PythonEvaluator;
struct GoEvaluator;

impl LanguageSpecificEvaluator for JavaEvaluator {
    fn matcher(&self) -> Box<dyn CallableMatcher> {
        Box::new(JavaCallableMatcher::new())
    }

    fn generate_uris(
        &self,
        template: &str,
        analysis: &AnalysisResult,
        _enums: &HashMap<String, Vec<String>>,
    ) -> Vec<String> {
        java_generate_target_uris(template, analysis)
    }
}

impl LanguageSpecificEvaluator for PythonEvaluator {
    fn matcher(&self) -> Box<dyn CallableMatcher> {
        Box::new(PythonCallableMatcher::new())
    }

    fn generate_uris(
        &self,
        template: &str,
        analysis: &AnalysisResult,
        enums: &HashMap<String, Vec<String>>,
    ) -> Vec<String> {
        python_generate_target_uris(template, analysis, enums)
    }
}

impl LanguageSpecificEvaluator for GoEvaluator {
    fn matcher(&self) -> Box<dyn CallableMatcher> {
        Box::new(GoCallableMatcher::new())
    }

    fn generate_uris(
        &self,
        template: &str,
        _analysis: &AnalysisResult,
        _enums: &HashMap<String, Vec<String>>,
    ) -> Vec<String> {
        let mut uris = vec![template.to_string()];
        if let Some((base, _query)) = template.split_once('?') {
            uris.push(base.to_string());
        }
        uris.sort();
        uris.dedup();
        uris
    }
}

static JAVA_EVALUATOR: JavaEvaluator = JavaEvaluator;
static PYTHON_EVALUATOR: PythonEvaluator = PythonEvaluator;
static GO_EVALUATOR: GoEvaluator = GoEvaluator;

pub(crate) fn evaluation_for(language: Language) -> &'static dyn LanguageSpecificEvaluator {
    match language {
        Language::Java => &JAVA_EVALUATOR,
        Language::Python => &PYTHON_EVALUATOR,
        Language::Go => &GO_EVALUATOR,
    }
}
