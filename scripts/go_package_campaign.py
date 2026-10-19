"""Broader empirical validation, reusing the existing isolated replay boundary."""

from collections import Counter
import json
import subprocess


def inspect_projects(
    runner, benchmark_root, output, native, timeout, go="go", revisions=None
):
    """Inspect every checkout; unsupported and unprepared projects stay visible."""
    output.mkdir(parents=True, exist_ok=True)
    results = []
    revisions = revisions or {}
    for source in sorted(p for p in benchmark_root.iterdir() if p.is_dir()):
        for requested in sorted(
            revisions.get(source.name, [None]), key=lambda r: r or ""
        ):
            suffix = (
                "-" + requested[:12] if len(revisions.get(source.name, [])) > 1 else ""
            )
            artifacts = output / (source.name + suffix)
            artifacts.mkdir()
            row = {
                "project": source.name,
                "status": "inspection_failed",
                "revision": requested,
                "revision_kind": "replay_baseline" if requested else "current_head",
            }
            try:
                head = runner.git(source, "rev-parse", "HEAD")
                revision = runner.git(source, "rev-parse", requested or head)
                row.update(revision=revision, checkout_head=head)
                tracked = runner.git(
                    source, "ls-tree", "-r", "--name-only", revision
                ).splitlines()
                modules = [f for f in tracked if f == "go.mod" or f.endswith("/go.mod")]
                row["module_files"] = modules
                if modules != ["go.mod"]:
                    row.update(
                        status="unsupported_layout",
                        reason="requires one module at the Git root; nested modules are unsupported",
                    )
                elif not any(f.endswith("_test.go") for f in tracked):
                    row.update(status="no_tests", reason="no committed Go test files")
                else:
                    root = artifacts / "module"
                    runner.git(
                        benchmark_root,
                        "clone",
                        "--no-hardlinks",
                        "--no-checkout",
                        str(source),
                        str(root),
                    )
                    runner.git(root, "checkout", "--detach", revision)
                    context = runner.driver_context(go, root, artifacts, timeout)
                    if runner.command(
                        [
                            go,
                            "list",
                            "-mod=readonly",
                            "-deps",
                            "-test",
                            "-json",
                            "./...",
                        ],
                        root,
                        artifacts,
                        "readiness-list",
                        timeout,
                        context,
                    ):
                        row.update(
                            status="package_loading_failed",
                            reason="offline loading failed; inspect readiness-list.stderr (no dependencies downloaded)",
                        )
                    elif runner.command(
                        [
                            go,
                            "test",
                            *context["build_flags"],
                            "-count=1",
                            "-json",
                            "./...",
                        ],
                        root,
                        artifacts,
                        "readiness-baseline",
                        timeout,
                        context,
                    ):
                        row.update(
                            status="baseline_failed",
                            reason="full baseline suite failed; not eligible for mutation grading",
                        )
                    elif runner.git(root, "status", "--porcelain"):
                        row.update(
                            status="baseline_changed_inputs",
                            reason="baseline execution changed committed/untracked inputs",
                        )
                    else:
                        status = runner.command(
                            [
                                native,
                                "--mode",
                                "package-inventory",
                                "--source-dir",
                                root,
                                "--timeout",
                                f"{timeout}s",
                            ],
                            root,
                            artifacts,
                            "readiness-evidence",
                            timeout + 5,
                            context,
                        )
                        evidence = json.loads(
                            (artifacts / "readiness-evidence.stdout").read_text()
                        )
                        if (
                            status
                            or evidence.get("schema_version") != 1
                            or evidence.get("provider_id") != "go-native"
                            or evidence.get("status") != "ok"
                            or evidence.get("complete") is not True
                            or evidence.get("blockers")
                        ):
                            row.update(
                                status="package_evidence_unavailable",
                                reason="native metadata is incomplete, blocked, or unsupported",
                                blockers=evidence.get("blockers", []),
                            )
                        elif not evidence.get("execution_packages"):
                            row.update(
                                status="no_active_test_binaries",
                                reason="no active test binaries in recorded context",
                            )
                        else:
                            row.update(
                                status="ready",
                                execution_binaries=len(evidence["execution_packages"]),
                                context=context,
                            )
            except (
                ValueError,
                RuntimeError,
                OSError,
                subprocess.SubprocessError,
            ) as error:
                row["reason"] = str(error)
            runner.save(artifacts / "result.json", row)
            print(f"readiness {source.name}: {row['status']}", flush=True)
            results.append(row)
            runner.save(output / "readiness.json", results)
    return results


def expanded_cases(runner):
    """Fixed, reviewable mutations, not chosen after observing test failures."""
    cases = runner.cases()

    def micro(name, service, before, after):
        return runner.Case(
            name,
            "go-micro-services",
            runner.MICRO,
            f"internal/services/{service}/{service}.go",
            before,
            after,
            (runner.SERVICE + service,),
        )

    cases += [
        micro(*row)
        for row in [
            (
                "geo-radius-inverted",
                "geo",
                "d <= maxSearchRadius",
                "d > maxSearchRadius",
            ),
            (
                "geo-order-reversed",
                "geo",
                "candidates[i].dist < candidates[j].dist",
                "candidates[i].dist > candidates[j].dist",
            ),
            (
                "geo-distance-half",
                "geo",
                "earthRadiusKm * 2 *",
                "earthRadiusKm * 0.5 *",
            ),
            ("geo-radian-scale", "geo", "math.Pi / 180.0", "math.Pi / 90.0"),
            (
                "geo-latitude-add",
                "geo",
                "(lat2 - lat1) * toRad",
                "(lat2 + lat1) * toRad",
            ),
            (
                "geo-longitude-add",
                "geo",
                "(lon2 - lon1) * toRad",
                "(lon2 + lon1) * toRad",
            ),
            (
                "geo-empty-results",
                "geo",
                "out = append(out, c.point)",
                "_ = c.point; out = nil",
            ),
            (
                "profile-missing-id",
                "profile",
                "return s.profiles[id]",
                'return s.profiles["missing-control-id"]',
            ),
            (
                "profile-empty-results",
                "profile",
                "res.Hotels = append(res.Hotels, h)",
                "res.Hotels = nil",
            ),
            (
                "profile-empty-loaded-map",
                "profile",
                "return profiles",
                "return map[string]*profile.Hotel{}",
            ),
            (
                "rate-condition-inverted",
                "rate",
                "if s.rateTable[stay] != nil",
                "if s.rateTable[stay] == nil",
            ),
            (
                "rate-empty-loaded-map",
                "rate",
                "return rateTable",
                "return map[stay]*rate.RatePlan{}",
            ),
            (
                "search-empty-results",
                "search",
                "res.HotelIds = append(res.HotelIds, ratePlan.HotelId)",
                "_ = ratePlan; res.HotelIds = nil",
            ),
            ("search-latitude-zero", "search", "Lat: req.Lat,", "Lat: 0,"),
            ("search-longitude-zero", "search", "Lon: req.Lon,", "Lon: 0,"),
            (
                "frontend-date-order",
                "frontend",
                "!outParsed.After(inParsed)",
                "outParsed.After(inParsed)",
            ),
            ("frontend-date-limit", "frontend", "30*24*time.Hour", "0*24*time.Hour"),
            ("frontend-logo-default", "frontend", "if img.Default", "if !img.Default"),
            (
                "frontend-address-separator",
                "frontend",
                'strings.Join(clean, ", ")',
                'strings.Join(clean, " | ")',
            ),
            (
                "frontend-empty-features",
                "frontend",
                '"features": fs,',
                '"features": fs[:0],',
            ),
            (
                "frontend-zero-ratings",
                "frontend",
                "ratings[row.ID] = row.Rating",
                "ratings[row.ID] = 0",
            ),
            (
                "frontend-health-status",
                "frontend",
                'writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})',
                'writeJSON(w, http.StatusCreated, map[string]string{"status": "ok"})',
            ),
        ]
    ]
    for name, source, before, after in [
        (
            "coffeeshop-invalid-parse",
            "entity.go",
            "uuid.Parse(s)",
            'uuid.Parse("not-a-uuid")',
        ),
        (
            "coffeeshop-parse-nil",
            "entity.go",
            "id, err := uuid.Parse(s)\n\n\treturn id, err",
            "return uuid.Nil, nil",
        ),
        (
            "coffeeshop-fixed-id",
            "entity.go",
            "return uuid.New()",
            'return uuid.MustParse("00000000-0000-0000-0000-000000000001")',
        ),
        (
            "coffeeshop-empty-events",
            "aggregate_root.go",
            "return ar.domainEvents",
            "return nil",
        ),
        (
            "coffeeshop-duplicate-events",
            "aggregate_root.go",
            "append(ar.domainEvents, e)",
            "append(ar.domainEvents, e, e)",
        ),
    ]:
        cases.append(
            runner.Case(
                name,
                "go-coffeeshop",
                runner.COFFEE,
                "internal/pkg/shared_kernel/" + source,
                before,
                after,
                (runner.SHARED,),
            )
        )
    return cases


def summarize(results, readiness):
    good = [r for r in results if r.get("success")]
    reports = [
        r
        for r in results
        if r.get("selection_mode") in ("conservative_packages", "conservative_all")
    ]
    packages = [
        r for r in reports if r.get("selection_mode") == "conservative_packages"
    ]
    observed = [r for r in results if "observed_failing_binaries" in r]
    observed_packages = [r for r in packages if "observed_failing_binaries" in r]
    timings = []
    for row in results:
        d = row.get("durations_seconds", {})
        if all(k in d for k in ("analysis", "mutant-test", "selected-test")):
            combined = d["analysis"] + d["selected-test"]
            timings.append(
                {
                    "case": row.get("case"),
                    "full_suite_seconds": d["mutant-test"],
                    "analysis_seconds": d["analysis"],
                    "selected_seconds": d["selected-test"],
                    "analysis_plus_selected_seconds": round(combined, 3),
                    "net_saving_seconds": round(d["mutant-test"] - combined, 3),
                }
            )
    successful_packages = [r for r in packages if r.get("success")]
    projects = sorted({r["project"] for r in successful_packages})
    inconclusive = sum(
        r.get(
            "inconclusive",
            not r.get("success") and r.get("omitted_failing_binaries") is None,
        )
        for r in results
    )
    limitations = []
    if len(projects) < 3:
        limitations.append(
            "The three-project breadth target was not met; services from one repository are not independent projects."
        )
    if len(successful_packages) < 30:
        limitations.append(
            "The target of 30 successful package-selection body mutations was not met."
        )
    return {
        "success": bool(results) and all(r.get("success") for r in results),
        "scope": "linked-binary",
        "soundness_claim": "empirical evidence, not a general soundness proof",
        "readiness": readiness,
        "readiness_counts": dict(Counter(r["status"] for r in readiness)),
        "results": results,
        "evaluated_projects": projects,
        "package_selection_cases": len(packages),
        "reduced_cases": sum((r.get("excluded_binaries") or 0) > 0 for r in packages),
        "successful_controls": len(good),
        "fallback_cases": sum(
            r.get("selection_mode") == "conservative_all" for r in reports
        ),
        "failure_producing_package_cases": sum(
            bool(r["observed_failing_binaries"]) for r in observed_packages
        ),
        "captured_failure_cases": sum(
            bool(r["observed_failing_binaries"])
            and r.get("omitted_failing_binaries") == 0
            and not r.get("inconclusive", False)
            for r in observed
        ),
        "surviving_mutants": sum(not r["observed_failing_binaries"] for r in observed),
        "full_suite_observed_cases": len(observed),
        "omitted_failing_binaries": sum(
            r.get("omitted_failing_binaries") or 0 for r in results
        ),
        "inconclusive_cases": inconclusive,
        "invalid_mutants": sum(
            r.get("mutation_outcome") == "invalid_mutant" for r in results
        ),
        "failed_cases": sum(not r.get("success") for r in results),
        "coverage_target": {"projects": 3, "body_mutations": 30},
        "coverage_target_met": len(projects) >= 3 and len(successful_packages) >= 30,
        "limitations": limitations,
        "timing_note": "Sequential warm-cache observations, not randomized performance estimates; negative savings are retained.",
        "timed_cases": timings,
    }


def run(runner, args):
    root = args.benchmark_root.resolve()
    manifest = expanded_cases(runner)
    runner.save(args.output_dir / "manifest.json", [vars(c) for c in manifest])
    revisions = {}
    for case in manifest:
        revisions.setdefault(case.project, set()).add(case.baseline)
    readiness = inspect_projects(
        runner,
        root,
        args.output_dir / "readiness",
        args.go_callgraph_binary.resolve(),
        args.analysis_timeout_seconds,
        revisions=revisions,
    )
    by_revision = {(r["project"], r["revision"]): r for r in readiness}
    results = []
    for case in manifest:
        if by_revision.get((case.project, case.baseline), {}).get("status") != "ready":
            result = {
                "case": case.name,
                "project": case.project,
                "success": False,
                "omitted_failing_binaries": None,
                "error": "project readiness did not pass; see readiness artifacts",
            }
        else:
            result = runner.evaluate_case(
                case,
                root,
                args.output_dir,
                args.go_callgraph_binary.resolve(),
                args.go_test_impact_binary.resolve(),
                args.analysis_timeout_seconds,
            )
        results.append(result)
        runner.save(args.output_dir / "summary.json", summarize(results, readiness))
    summary = summarize(results, readiness)
    runner.save(args.output_dir / "summary.json", summary)
    print(
        f"Campaign: {len(results)} cases; {len(summary['evaluated_projects'])} projects; "
        f"{summary['reduced_cases']} reduced; {summary['omitted_failing_binaries']} omitted failures; "
        f"{summary['inconclusive_cases']} inconclusive; breadth target met={summary['coverage_target_met']}",
        flush=True,
    )
    for limitation in summary["limitations"]:
        print("LIMITATION: " + limitation, flush=True)
    return summary
