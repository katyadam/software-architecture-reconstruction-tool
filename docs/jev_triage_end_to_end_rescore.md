# Jev triage — end-to-end re-score (edge-level P/R)

Self-contained brief for a cold session. Goal: does Jev residual triage
improve the **final SDG edges**, scored against the business oracles?
Task 1 (`docs/jev_triage_generalization_check.md`) only measured the triage
verdict. This task measures what the whole pipeline produces.

- **Branch:** `sage-rework` (V5 triage, default `JEV_THRESHOLD` 0.8; task 1
  committed as `4fb2a11`).
- **Scope:** measurement only. Do not change triage, matcher, Sage, or the
  synthesizer. If something looks wrong, record it with its cause. Do not fix
  it in this task.

---

## 1. Background

Pipeline for a REST call whose `target_uri` is not a URL after symbolic
evaluation (a **residual**):

```
triage (Jev is_http, only with --typesafe)
  -> NonEdge: dropped, target_uri untouched
  -> NeedsResolution:
       deterministic matcher (identifier/class -> service)
         -> hit: target_uri rewritten onto the service URL
         -> miss: Sage (Ollama closed-set LLM) -> rewrite or abstain
synthesizer: target_uri -> endpoint -> service edge (sdg.json)
```

- Code: `extractor-runtime/src/pipeline/pass3/llm_enhance/dispatch.rs`
  (`resolve_one`), `residual_edge_filter.rs` (triage).
- **Without `--typesafe`, every residual reaches the matcher and Sage.** No
  non-edge filter is left (old lexical/structural filters were removed in
  `8d027e5`). Earlier, the matcher fired on non-edge noise (74/77 on empaia,
  `docs/sage_rework_results.md` decision log 2026-06-29). The hypothesis is
  that Jev removes those false edges.
- **CLI flags** (`cli/src/main.rs`, `cli/src/lib.rs`):
  - Without `--llm`, the residual path does not run at all (pure static).
  - `--typesafe` is **ignored** without `--llm`.
  - `--llm-url http://127.0.0.1:9/v1` (dead port) = matcher only, because Sage
    fails instantly.

Task 1 triage results (V5, t = 0.8):

| corpus | residuals | HTTP | P / R |
|---|---|---|---|
| empaia | 128 | 20 | 1.00 / 1.00 |
| train-ticket (Java) | 4 | 4 | 1.00 / 1.00 |
| train-ticket-polyglot | 45 | 39 (16 = ASGI in-process test calls) | 1.00 / 1.00 |

Last published edge-level numbers (all **before** Jev existed):

| corpus / run | P / R | TP FP FN | source |
|---|---|---|---|
| empaia, no LLM, constants + scrape | 0.87 / 0.81 | 13 / 2 / 3 | `sage_rework_results.md` (2026-07-31) |
| empaia, LLM + constants + scrape | 0.74 / 0.88 | 14 / 5 / 2 | `sage_next_steps.md` (stale, pre-typed-edges) |
| train-ticket Java | 1.00 / 0.95 | 87 / 0 / 5 | `sage_next_steps.md` |
| train-ticket polyglot + LLM | 0.86 / 0.77 | 71 / 12 / 21 | `sage_next_steps.md` |

The code has moved since these were measured. **Do not compare to them.**
Re-measure every arm on the current HEAD.

## 2. Arms

All arms run on the same commit with the same corpus flags. Only the flags
below change.

| arm | flags | what it isolates |
|---|---|---|
| **N** | (no `--llm`) | static baseline |
| **D** | `--llm --llm-url http://127.0.0.1:9/v1` | matcher on all residuals |
| **DJ** | D + `--typesafe` | matcher on Jev-passed residuals |
| **S** | `--llm` (live Ollama) | matcher + Sage on all residuals |
| **SJ** | S + `--typesafe` | matcher + Sage on Jev-passed residuals |

- Headline comparisons: **D vs DJ** (no LLM noise) and **S vs SJ**.
- N shows what the residual path adds at all.
- **S and SJ need Ollama** (`qwen2.5-coder:7b` at `http://localhost:11434/v1`).
  It was not running when this brief was written
  (`curl localhost:11434/v1/models` returned nothing).
  - Check first. If it is unreachable, **ask the user**. Do not install
    anything.
  - If the user can't start it, run N/D/DJ only and record S/SJ as blocked.
- Repetitions:
  - Sage is sampled. Run S and SJ **3 times each** and report every
    repetition, plus the median.
  - N, D and DJ: 1 run each. Jev drift is about ±0.02, far inside the
    0.72–0.96 gap, so one run is enough.

## 3. Corpora and commands

```bash
cargo build --release -p cli
OUT=jev_triage_runs/e2e-v5     # gitignored; also keep sdg.json per run
```

| corpus | `-p` | `-c` | extra |
|---|---|---|---|
| empaia | `../empaia` | `config/configurations/local-empaia-config.json` | `-f config/constants/empaia-constants.json --scrape` |
| tt-polyglot | `../train-ticket-polyglot` | `config/configurations/local-train-ticket-polyglot-config.json` | `--scrape` |
| tt-java | `../train-ticket` | `config/configurations/local-train-ticket-config.json` | `--scrape` |

Per run:

```bash
JEV_LOG=$OUT/<corpus>-<arm>[-rN].triage.log \
RUST_LOG=info \
target/release/cli -p <P> -c <C> <extra> <arm flags> -o $OUT/<corpus>-<arm>[-rN] \
  2>&1 | tee $OUT/<corpus>-<arm>[-rN].log
```

- `JEV_LOG` only fills on arms with `--typesafe`.
- tt-java has only 4 residuals, all HTTP. Expect DJ == D and SJ == S. Run it
  as a no-op sanity check, using the N/D/DJ arms only.
- Prerequisite: `TYPESAFE_API_KEY` set (the CLI loads `.env`).
- Before anything else, `git status` must be clean. Record the HEAD commit in
  the results. Every arm must run from that one binary.

## 4. Scoring

Edge-level scoring lives **outside** this repo, in `~/fi-muni/sar-compare`:

- empaia: `ground_empaia.py` checks the oracle
  `empaia-ground-truth/sdg-edges.json` (16 business edges; it asserts 16).
  `score(oracle, tool)` returns tp/fp/fn/P/R.
- train-ticket: `ground_sdg.py` reads `rest-edges.yaml` (92 edges),
  `services.yaml` and `endpoint-edges.yaml`, and `metrics(oracle, tool)`
  scores them.
  - **Path bug:** its `ORACLE_DIR` resolves to `~/fi-muni/train-ticket/...`,
    which does not exist. The real one is
    `~/research/train-ticket/benchmark-ground-truth`.
  - Pass that path to `load_oracle(...)`. Do not edit sar-compare.
- Existing driver: `results/llm-typed-score_dual.py` (`usage: <empaia|polyglot|java>
  <sdg.json>...`). It prints the all-kinds and Business-only views.
  - It hardcodes `SC = ~/muni/sar-compare` (an old machine). Copy it to
    `$OUT/score_e2e.py`, then fix `SC` -> `~/fi-muni/sar-compare` and the
    train-ticket oracle path.
  - Keep the scoring logic identical.
- **Headline = Business-only view.** It excludes the `TestOrigin` /
  reflexive kinds, as in Phase 4. Report the all-kinds view too.
- Sanity check before trusting the scorer: score arm N on empaia. Read each
  FP/FN and confirm it is plausible. A new run is not a valid reference point
  for an old one: see the 2026-07-31 retraction in `sage_rework_results.md`.

## 5. What to measure per run

1. Edge-level P / R / TP / FP / FN, in both views.
2. FP and FN edge lists (Business view).
3. Funnel, read from the `info` log lines in `dispatch.rs`:
   - `jev: N non-edge(s), E error(s) of R residual(s)`
   - `deterministic resolver: resolved X of Y cross-service residual(s)`
   - `Number of REST calls evaluated with LLM: Q` (Sage calls, which is the
     cost Jev should cut)
4. Extraction wall time (the `Extraction:` line). On Jev arms, also the
   `[triage] wall` line.
5. Jev errors. If any arm has more than 0, the run is not clean. Rerun it once
   and note it.

## 6. Attribution (the important part)

Numbers alone are not enough. For every edge that **differs** between D and
DJ (and between S and SJ), find the residual(s) that caused it.

- Diff `sdg.json` connections (source, target) between the two arms. For each
  changed edge, list its `requests` (method + uri) and trace each back to a
  call site: grep the triage log or the extractor output for the uri/file.
- Label each change:
  - **removed FP**: Jev dropped a non-edge that the matcher or Sage had turned
    into a false edge. This is the expected win.
  - **lost TP**: Jev dropped a real HTTP call. This should not happen given
    task 1 R = 1.00. If it does, it is the key finding.
  - **unchanged FP**: the FP survives Jev, so it comes from a resolution
    mistake, not a triage mistake. Example: the known empaia Category D
    host -> service misresolutions in `docs/sage_next_steps.md` item 1. Jev
    cannot fix those, so name them.
  - **Sage variance**: the edge differs between repetitions of the same arm.
    It is not attributable to Jev.
- **polyglot ASGI test calls** (open question from task 1): the 16
  `client.get/post` calls in `tests/test_api.py` pass triage. Check what they
  become:
  - Their `target_uri` is a bare path; the `base_url` lives in the
    `AsyncClient(...)` line.
  - Do they resolve to an edge, and to which service?
  - Is that edge typed `TestOrigin` (excluded from Business) or `Business`?
  - Report the facts. Do not change labels or code.

## 7. Outputs

1. `jev_triage_runs/e2e-v5/results.md`:
   - HEAD commit, date, Ollama model and status, and the exact commands.
   - One table per corpus: rows = arms (and S/SJ repetitions plus median);
     columns = P, R, TP, FP, FN (Business), P/R (all kinds), Jev non-edges,
     matcher hits, Sage calls, extraction wall time.
   - The section 6 attribution table: edge, arm diff, label, causing call
     site(s).
   - The ASGI finding.
   - A "Reading" section with the verdict (see section 8).
2. A dated entry in `docs/sage_rework_results.md` with the headline numbers,
   the verdict and the decision taken.
3. Update the "Baseline best results" block in `docs/sage_next_steps.md`:
   replace the stale numbers with these, marked with the date and commit.
4. Commit the doc changes on `sage-rework`. Suggested message:
   `Measure end-to-end SDG edges with Jev triage`.
   - `jev_triage_runs/` is gitignored. The committed docs must carry every
     number that matters.

## 8. Verdict criteria

- **Jev helps:** DJ precision > D precision with no lost TP, and Sage calls
  drop on SJ vs S.
  - Next: task 3, Jev `Choice` for the target service, replacing Sage.
  - The 3 Sage reps of S/SJ are the baseline task 3 must beat.
- **Neutral:** edges identical between D and DJ (and S/SJ within Sage
  variance). The FPs are resolution errors, not triage errors.
  - Next: still task 3, plus the Category D host -> service misresolutions.
  - Record the Sage-call and time savings anyway.
- **Jev hurts:** any lost TP. Diagnose it from the triage log state (score,
  missing field).
  - Do **not** tune the question or threshold here.
  - Propose the fix as the next triage version, to be measured on all three
    corpora.

## 9. Constraints

- Follow the repo `CLAUDE.md`. This task changes no source code. The only new
  script is the copied scorer, kept under `jev_triage_runs/`.
- Do not edit `~/fi-muni/sar-compare` or the oracle files.
- Do not commit `.env` or anything under `jev_triage_runs/`.
- Keep reports concise. Use `->`, never the arrow glyph.
