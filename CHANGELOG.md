# Unreleased

- Python: no crash on a lone quote part in a REST call target (#56)
- Python: REST call URI combinations capped at 256 per call (#57)
- Python: `f` prefix stripped only from real f-strings (#58)
- Python: call arguments split by syntax, `url=` preferred as REST target (#59)
- Deterministic callee choice when several callables match a call (#62, partial; see #101)
- Go: major-version imports (`chi/v5`) use the package name as alias (#68)
- Syntax tree depth and source size limits; the CLI skips files it can't extract instead of aborting (#70)
- Java: class route prefix only from the enclosing `@RequestMapping`/`@Path`, `method=` read, `@FeignClient` skipped, prefix arrays (#71)

# 0.6.0

- Multi pass approach for better precision
- Class attribute default values resolution
- Module-level symbolic evaluation for constant propagation
- Environment variables scraping

# 0.5.0

- CLI mode
- Enum Extraction

# 0.4.0

- Added Constant Scanner

# 0.3.0

- Added Java Extractor - Spring Strategy
- New crate for various static analyses
  - Using Symbolic Evaluation for Java Extractor
- Successful run for train-ticket
- Improving SDG synthetization precision

# 0.2.0

- Seperated runtime for python-extractor
- Added Commit as a primary metadata for saved set of views
- Fixed invoked on to also compare Scopes of methods
- Inter-Service Context Map

# 0.1.0

- Context Map creation, storing and retrieving
- SDG creation, storing and retrieving
- IMCG creation, storing and retrieving
- Basic Call Graph creation using basic type inference
- Python code elements extractor
- Metadata manager
- Object Storage for passing data from extraction to synthesization
- Docker-compose deployment option
