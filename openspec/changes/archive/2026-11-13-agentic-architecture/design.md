## Context

See proposal.md - Why. Source: artifacts "VOYANTCLAIR Agentic SDG" and "Matches Type Tree", plus the review that found eight holes. Branch `agentic-sdg`. Built so far: `agentic/models` (package `agentic-models`), `InteractionKind` moved to root `models`.

Current baseline (`cli`, frozen): extractors -> extractor-runtime (LLM inside pass 3) -> synthesizer (static matching, then Jev tie-break) -> `sdg.json`. Scores on 2026-11-13, no `--scrape`:

| Subject | static | best System One mode |
|---|---|---|
| TT Java | 1.000/.967/.983 | same |
| TT Polyglot | 1.000/.935/.966 | same |
| Empaia | .941/.941/.941 | 1.000/.941/.970 (`--jev-match`) |

## Goals / Non-Goals

**Goals:**
- One recorded architecture for all agentic follow-up changes.
- Static analysis supplies facts. LLM stages match, verify, rematch.
- New language needs extractor work only, no matcher or verifier change.

**Non-Goals:**
- MCP server. Synthesizer reuse. Context Map or IMCG.
- Changes to baseline `cli` behaviour.
- Scoring non-HTTP edges before an oracle exists.
- Impact analysis, event vs command typing.

## Decisions

### Pipeline
```
repo + config
  -> extractors (unchanged)
  -> extractor-runtime passes 1-3, no LLM -> ProjectIR + evaluated URLs
  -> matcher (Jev)                      -> match/  Matches
  -> verifier  (--verify, system two)   -> keep/drop each link
  -> rematcher (--rematch, system two)  -> unmatched initiators only
  -> final/sdg.json + final/sdg.legacy.json -> score.py
```
Each stage writes `Matches` to disk. Verify and rematch reruns spend nothing on Jev. Flags off: last stage run is final.

Alternative: LLM inside extraction (current DDFA). Rejected: mixes resolution into extraction, can't score stages apart.

### Crates
```
agentic/
  cli/        agentic-cli     -> matcher, verifier, rematcher
  models/     agentic-models  -> models
  agent-loop/                 -> agentic-models
  matcher/                    -> agent-loop, agentic-models
  verifier/                   -> agent-loop, agentic-models
  rematcher/                  -> matcher, agent-loop, agentic-models
```
- One workspace. Members glob `"agentic/*"` (now `"agentic/models"`; switched in `agentic-models-verdicts`).
- `Prior`, `Hints`, `Outcome` live in matcher. `Verdict` and `LinkId` go to agentic-models, because the CLI saves verdicts. `Context` lives in agent-loop.
- agent-loop uses point types only. Rule, not compiler-enforced.
- Copied, not depended on: `matching.rs::candidates()` (free to diverge), service assignment (segment match, `Option` result, log unassigned).
- `classify()` semantics must stay equal to synthesizer, or scores drift. Candidate for a later move to `models`.
- Needed from extractor-runtime: entry point exposing `ProjectIR`, public `llm_enhance/facts`.

### Connection points
Every dependency side is a `ConnectionPoint`: id, service, side, protocol, address, element. `address` is the small match key and may change. `element` is the original extraction, never mutated. Diffing them shows what matcher changed.

`PointId` is a readable string (`file#function_hash#kind#raw[#start-end]`). Alternative: hash of the same parts (artifact). Rejected: equally stable, unreadable in logs.

Protocol follows the element. gRPC already arrives as `MessageEdge` with `CommunicationProtocol::Grpc` (hole 8), so `Message` does not mean channel.

### Matching per point
Per kind, initiators first:
1. Triage (Jev): is this really a call of this kind? yes/no/other. Rejects -> `rejected.json`. Named `triage` to avoid clash with `classify()`.
2. `similar()` candidates. Empty: widen filter once.
3. One strong match: accept, no LLM, `MatchSource::Static`.
4. Else Jev loop: pick service, then endpoint. Closed-set answers only (candidate id or none). Every answer cites fact ids.
5. Service holds, no endpoint: link with `receiver: None`.
6. Else `Unmatched` with reason.

`--both-sides <kinds>` resolves receivers only for listed kinds with unmatched internal initiators. Receiver loop never sees the call URL.

`Outcome = Picked | Resolved(Address) | External | Unmatched` (hole 3). Channels resolve then join on name, so one publisher can yield many links. One `Unmatched` per initiator, merged `banned`.

`fn match_point(point, prior: Option<Prior>, model) -> Outcome`. Matcher passes `None`, rematcher `Some`. `Prior { banned, kept, service, hints }` changes prompt and candidates only.

Invariant: when `receiver` is `Some`, `receiver.service == Dependency.target`. Enforced by the matcher writer.

### Tool protocol
One schema-enforced JSON object per step: `{tool, args}` or `{final, ...}`. Jev already returns JSON from choice questions. No native tool calling needed. Spike dropped (hole 6).

Alternative: provider-native tools. Rejected: Jev lacks them; ties stages to one provider.

Tools, all read-only, over `ProjectIR` or the point index:

| Tool | Gives | Source |
|---|---|---|
| similar | top receivers, other service, same method | copy of `matching.rs` |
| service_receivers | all receivers of one service for current kind | point index |
| define, callers, receiver, constant, open_file | code facts | `llm_enhance/facts` |
| imports | import neighbours | `ProjectIR.import_graph` |

Service arg is a closed set from config. Kind is filled by matcher, not Jev. Output rows are compact, with point ids.

### Evidence (holes 1, 2)
Runtime numbers each tool result as a fact. Schema allows only fact ids returned in this session. Evidence stores the whole fact, since ids die with the session. Static matches get a runtime fact too. Jev never writes file or line, so `Endpoint` and `MessageEdge` need no spans.

Alternative: file + line citations. Rejected: model writes the number as text; endpoints lack lines.

### Verify and rematch
- Verifier iterates links only (pairs and `receiver: None`). Writes `Verdict { link, decision: Keep|Drop|Unverified, reason, evidence }`.
- A weak citation alone never drops a link. Drop must cite facts from verifier's own session.
- Drop: initiator -> unmatched, reason `DroppedByVerifier`, receivers of that service banned.
- Keep/Unverified on `receiver: None`: rematch searches only `Dependency.target`.
- Rematcher input: unmatched initiators, dropped initiators, `receiver: None` links. New pairs are not re-verified.
- Budgets: matcher max 6 steps, verifier 0 or 3 (compared).

### Excluded and failures (holes 4, 5)
- `Matches.excluded` with `ExcludedReason::External`. Audit a sample per run, since a wrong `External` is a silent false negative.
- `UnmatchedReason::ModelError` for timeout, error status, unparsable answer.
- agent-loop appends a JSONL record per point: stage, prompt hash, tool calls, answer, tokens. Enables resume after crash and run-to-run spread.
- Context size set explicitly. Returned prompt tokens compared with expected; mismatch fails the call (Ollama silent truncation, 2026-11-06).

### Outputs
| File | Writer |
|---|---|
| `match/sdg.json`, `match/sdg.legacy.json` | matcher |
| `match/rejected.json`, `match/unmatched.json`, `match/metrics.json` | matcher |
| `final/sdg.json`, `final/sdg.legacy.json` | last stage |
| `final/verdicts.json` | verifier |

Legacy file keeps the current `connections` shape, HTTP only, plus `match`, `confidence`, `evidence` per request. `score.py` runs unchanged.

### Edge direction
Initiator -> receiver. MQ: publisher -> subscriber. Broker stays in `Address::Channel.broker`, not a node.

### Scope cut
HTTP first. Channel types stay in the model. Oracles are REST-only, so channel matching waits. Overrides artifact's "HTTP and channels together".

### Models
- Jev (System One, TypeSafe API): matcher.
- System two: any OpenAI-compatible endpoint with JSON schema, `--system-two-url`, `--system-two-model`, key from env. Plumbing on free tier with `subset-train-ticket-config.json`. Evaluation on paid DeepSeek V4 Flash. Open models later on school GPUs.
- Never start local Ollama by default.

### Evaluation arms
Static baseline (pinned commit), matcher, `+--both-sides`, `+ablation`, `+--verify`, `+--rematch`, `+both`, system two as main model. Corpora: empaia, TT Java, TT Polyglot, one unseen. Report P/R/F1, tokens, run spread.

## Risks / Trade-offs

- [Score blind to receiver errors: oracle is service-level (hole 7)] -> hand-label a link sample per corpus; pick unseen corpus with its own oracle.
- [Little headroom: TT misses are dropped calls, not wrong picks] -> report empaia and unseen corpus first.
- [Invisible calls: matcher sees only extracted points] -> spec inference and route-prefix work stay useful.
- [Candidate recall: truth outside the set can't be picked] -> soft filter, widen once, `service_receivers`.
- [`constants.rs:53` HashMap tie] -> empaia static was identical over 4 runs on 2026-11-13; fix before arm comparison if spread appears.
- [Memorization of train-ticket by system two] -> unseen corpus.
- [Wrongly dropped true pair stays lost] -> visible as lost recall in "matcher + both" arm.
- [Actix still compiles through extractor-runtime] -> accepted.

## Migration Plan

None. New crates only. Baseline `cli` untouched.

## Open Questions

- Shared-resource edges (DB, bucket) in the SDG, or out of scope?
- Verify budget 0 vs 3: decided by evaluation.
- Which unseen corpus.
