use std::collections::HashMap;

use go_extractor::extraction::restcalls::evaluation::uri_generator::generate_target_uris as go_generate_target_uris;
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

type MatcherFactory = fn() -> Box<dyn CallableMatcher>;
type UriGenerator = fn(&str, &AnalysisResult, &HashMap<String, Vec<String>>) -> Vec<String>;

struct EvaluatorStrategy {
    matcher_factory: MatcherFactory,
    uri_generator: UriGenerator,
}

impl LanguageSpecificEvaluator for EvaluatorStrategy {
    fn matcher(&self) -> Box<dyn CallableMatcher> {
        (self.matcher_factory)()
    }

    fn generate_uris(
        &self,
        template: &str,
        analysis: &AnalysisResult,
        enums: &HashMap<String, Vec<String>>,
    ) -> Vec<String> {
        (self.uri_generator)(template, analysis, enums)
    }
}

static JAVA_EVALUATOR: EvaluatorStrategy = EvaluatorStrategy {
    matcher_factory: || Box::new(JavaCallableMatcher::new()),
    uri_generator: |template, analysis, _enums| java_generate_target_uris(template, analysis),
};

static PYTHON_EVALUATOR: EvaluatorStrategy = EvaluatorStrategy {
    matcher_factory: || Box::new(PythonCallableMatcher::new()),
    uri_generator: python_generate_target_uris,
};

static GO_EVALUATOR: EvaluatorStrategy = EvaluatorStrategy {
    matcher_factory: || Box::new(GoCallableMatcher::new()),
    uri_generator: go_generate_target_uris,
};

pub(crate) fn evaluation_for(language: Language) -> &'static dyn LanguageSpecificEvaluator {
    match language {
        Language::Java => &JAVA_EVALUATOR,
        Language::Python => &PYTHON_EVALUATOR,
        Language::Go => &GO_EVALUATOR,
    }
}
