# Update Log

## 2026-04-16 -> 2026-04-30

- added closure capture evaluation - looking at closures and their parents - during the analysis of the closured function, injecting the parent environment (part of it)
- cross-file data-flow tracking (very simple, looking only at strings, with basic ITE logic) - resolving default class attributes (settings class) and module level variables (singletons)
- environment variables scraping from .env\* and docker-compose files - combining those with same name via heuristics (basically those from .env files have higher priority than docker-compose envvars)
- some refactorings, for example, building global callables map once and putting it in ProjectIR (2nd pass IR)

### LLM generated summary

- Added closure capture evaluation: receivers on `Expr::Call`, `Expr::Attr`, captured scopes from outer function bindings, and end-to-end tests covering closure-pattern REST call resolution.
- Introduced cross-file data-flow tracking of constants defined at module level, as class attributes, or via combinations of both. Documented the approach under `docs/`.
- Built the environment-variable scraping module (`env-scraper`) and wired it into both CLI and the full-stack flow through `constant-scanner`.
- Added migrations tagging constants with their source file and enforcing uniqueness on that tag.
- Refactored Pass2 to build the global callable map once and place it on `ProjectIR`, removing duplicated traversal logic across passes.

## 2026-04-30 -> 2026-05-10

- Fixing some issues with previous implementation:
  - parameters that have call-like structure (Dict()) were wrongly interpreted
  - dual-map callable map (one key name, one key hash) for looking up closure-like functions
  - similarity matching for class fields with no default value and env scraped variables
  - multi-dotted python import resolution bug - did not lookup correct files

- Created crate - sage - for communication with LLMs, together with query structure and system prompts

- Trying to tune llm REST call resolution to have the exact information it needs. Kinda hard when you think of that one rest call might need only single scope,second 3-4 files...

- Currently the precision for empaia with all the semantic analysis tuning and without LLM evaluation is 16/17. The last edge is the abstraction via passing url to some class object representing client calling APIs (E-CLIENT-ABSTRACTION), which is ideal task for LLM, but currently hard to deliver. Also there is some overaproximation

- LLM tuning - two improvements: - better semantic analysis that would retrive what is ideal to pass to the LLM - instantiation sites (resolving E-CLIENT-ABSTRACTION) or only those variables that are needed - better model - I can barely run qwen2.5-coder:7b which should be the best for 16GB RAM - I need more resources to run something better
  Combining these two improvements should: make the evaluation more precise + faster.

- First LLM run for empaia times:
  Extraction: 3350.434251816s aprx. 81 secs for each LLM call (reasonable target would be under 10 seconds)
  Synthesis: 1.728326835s

### LLM generated summary

- Bootstrapped the `sage` crate: facts model, LLM resolver, query/response structures, and a client targeting the OpenAI-compatible Ollama API.
- Implemented variable-map construction in `pass3/llm_enhance.rs` combining project constants, per-file attrs, and module consts, then wired it into the sage query payload.
- Added selection logic (`is_restcall_evaluated_enough`) to pick only REST calls that genuinely need LLM arbitration, avoiding wasted calls.
- Implemented semantically correct multi-dotted Python import resolution, including transitive imports, with tests covering the new behavior.
- Fixed call-vs-default-value misinterpretation for callable parameters and added a dual-key callable map so same-named closure functions inside other functions resolve correctly.

### Another options

- provide static analysis tools to some agents via MCP
- porovnat modely
- agent na generovani FE pro dalsi jazyk
- db of code patterns
