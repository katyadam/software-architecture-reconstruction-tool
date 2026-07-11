/// One configured microservice the LLM may choose as the call's target.
///
/// `name` is the config service id (returned verbatim as the answer); `url` is
/// its canonical base URL, shown to the model for disambiguation only.
pub struct CandidateService {
    pub name: String,
    pub url: String,
}

/// The category of resolution question sent to Sage.
///
/// The rework collapses the old six generative variants into a single
/// closed-set classification: pick exactly one configured service (or null).
pub enum QueryKind {
    ClassifyTargetService { candidates: Vec<CandidateService> },
}

/// A question to Sage, pairing the call-site context with a resolution kind.
pub struct SageQuery {
    pub kind: QueryKind,
    pub context: ClassifyContext,
}

/// Curated, precedence-ordered signals around a residual REST call site.
///
/// Rendered into the prompt strongest-first (client class) to weakest
/// (identifiers), mirroring the deterministic matcher's precedence.
pub struct ClassifyContext {
    /// The CALLER service; excluded as a possible answer (no self-loops).
    pub origin_service: String,
    /// Enclosing client class name, if any -- the strongest signal.
    pub client_class: Option<String>,
    /// Import module/name strings present in the call's source file.
    pub imports: Vec<String>,
    /// The residual `target_uri` expression that failed static resolution.
    pub expression: String,
    /// Identifiers appearing in the residual expression -- the weakest signal.
    pub operand_identifiers: Vec<String>,
}
