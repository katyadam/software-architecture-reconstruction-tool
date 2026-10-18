#!/usr/bin/env python3
"""Isolated, offline package-policy controls. Empirical evidence, not a proof."""
import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import shlex
import signal
import subprocess
import sys
import threading
import time


@dataclass(frozen=True)
class Case:
    name: str
    project: str
    baseline: str
    source: str
    before: str | None
    after: str | None
    expected_packages: tuple[str, ...] | None
    expected_mode: str = "conservative_packages"
    augment: str | None = None


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def protected_environment(context=None):
    env = dict(os.environ)
    env.update(GOENV="off", GOFLAGS="", GOWORK="off", GOTOOLCHAIN="local", GOPROXY="off", GOSUMDB="off")
    if context:
        overrides = context["environment"]
        if overrides.get("GOENV") != "off" or overrides.get("GOFLAGS") != "" or overrides.get("GOWORK") != "off" or overrides.get("GOPROXY") != "off":
            raise ValueError("recorded build context lost offline/protected settings")
        env.update(overrides)
    return env


def command(args, root, artifacts, label, timeout, context=None):
    """Argument arrays only; retain output and supervise the entire process group."""
    started = time.monotonic()
    print(f"{artifacts.name}: {label}", flush=True)
    with (artifacts / f"{label}.stdout").open("w") as stdout, (artifacts / f"{label}.stderr").open("w") as stderr:
        child = subprocess.Popen([str(a) for a in args], cwd=root, env=protected_environment(context), stdout=stdout, stderr=subprocess.PIPE, text=True, start_new_session=True)
        def collect():
            for line in child.stderr:
                stderr.write(line)
                if "_phase=" in line:
                    print(line.rstrip(), flush=True)
        collector = threading.Thread(target=collect, daemon=True)
        collector.start()
        try:
            status = child.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait()
            collector.join()
            child.stderr.close()
            raise RuntimeError(f"{label} exceeded control deadline {timeout}s; artifacts preserved")
        collector.join()
        child.stderr.close()
    save(artifacts / f"{label}.command.json", {"arguments":[str(a) for a in args], "status":status, "duration_seconds":round(time.monotonic()-started,3)})
    return status


def json_stream(path):
    data = path.read_text()
    decoder = json.JSONDecoder()
    values = []
    while data.strip():
        value, end = decoder.raw_decode(data.lstrip())
        values.append(value)
        data = data.lstrip()[end:]
    return values


def git(root, *args):
    out = subprocess.run(["git","-C",str(root),"-c","user.name=Go package control","-c","user.email=go-tia@example.test","-c","commit.gpgsign=false",*args], capture_output=True, text=True, timeout=60)
    if out.returncode:
        raise RuntimeError(f"git {args[0]}: {out.stderr.strip()}")
    return out.stdout.strip()


def commit(root, message):
    git(root,"add","--all")
    git(root,"commit","-qm",message)
    return git(root,"rev-parse","HEAD")


def driver_context(go, root, artifacts, timeout):
    if command([go,"env","-json"],root,artifacts,"go-env",timeout):
        raise RuntimeError("could not record effective local Go driver")
    env = json.loads((artifacts/"go-env.stdout").read_text())
    for key in ["GOVERSION","GOOS","GOARCH","CGO_ENABLED"]:
        if not env.get(key):
            raise ValueError(f"effective Go driver missing {key}")
    if env["GOOS"] != env.get("GOHOSTOS") or env["GOARCH"] != env.get("GOHOSTARCH"):
        raise ValueError("control cannot execute a cross-platform build context")
    features = {k:env[k] for k in ["GO386","GOAMD64","GOARM","GOARM64","GOMIPS","GOMIPS64","GOPPC64","GORISCV64","GOWASM"] if env.get(k)}
    overrides = {"GOENV":"off","GOFLAGS":"","GOWORK":"off","GOTOOLCHAIN":"local","GOPROXY":"off","GOSUMDB":"off","GOOS":env["GOOS"],"GOARCH":env["GOARCH"],"CGO_ENABLED":env["CGO_ENABLED"],"GOEXPERIMENT":env.get("GOEXPERIMENT","")}
    overrides.update(features)
    return {"go_version":env["GOVERSION"],"goos":env["GOOS"],"goarch":env["GOARCH"],"cgo_enabled":env["CGO_ENABLED"],"architecture_features":features,"goexperiment":env.get("GOEXPERIMENT",""),"build_tags":"","environment":overrides,"build_flags":["-mod=readonly"]}


def exact_argument(package, root):
    root = root.resolve()
    relative = package["relative_directory"]
    if not relative or "\\" in relative or "..." in relative or relative.startswith("/") or relative != "." and any(p in ("", ".", "..") for p in relative.split("/")):
        raise ValueError("not an exact module-relative execution directory")
    path = root / relative
    if not path.is_dir() or not path.resolve().is_relative_to(root.resolve()) or path.resolve() != path:
        raise ValueError("execution directory escapes or aliases verified checkout")
    return "." if relative == "." else "./" + relative


def execution_arguments(report, root):
    if report.get("schema_version") != 2 or report.get("requested_policy") != "packages" or report.get("declared_test_scope") != "linked-binary":
        raise ValueError("not a scoped schema-2 package report")
    if report["selection_mode"] == "conservative_all" and report.get("run_all_tests") is True:
        return ["./..."]
    if report["selection_mode"] != "conservative_packages" or report.get("execution_inventory_complete") is not True:
        raise ValueError("incomplete execution inventory cannot authorize exclusions")
    discovered = {p["import_path"]:p for p in report["discovered_execution_packages"]}
    if len(discovered) != len(report["discovered_execution_packages"]):
        raise ValueError("duplicate execution packages")
    decisions = report["selected_execution_packages"] + report["unselected_execution_packages"]
    names = [d["package"]["import_path"] for d in decisions]
    if len(set(names)) != len(names) or set(names) != set(discovered):
        raise ValueError("execution decisions do not partition inventory")
    for decision in decisions:
        package = decision["package"]
        if package != discovered[package["import_path"]] or decision["argument"] != exact_argument(package,root):
            raise ValueError("inconsistent exact execution argument")
    return [d["argument"] for d in report["selected_execution_packages"]]


def run_selected_tests(arguments, root, context, artifacts, timeout, go="go"):
    if not arguments:
        (artifacts/"selected-test.skipped").write_text("Eligible empty execution set: do not invoke bare go test.\n")
        return 0
    return command([go,"test",*context["build_flags"],"-count=1","-json",*arguments],root,artifacts,"selected-test",timeout,context)


def package_observation(items, changed):
    """Use actual Go-list instances/ImportMap, never bracket or .test parsing."""
    nodes = {p["ImportPath"]:p for p in items}
    groups = {}
    owners = set()
    for identity, p in nodes.items():
        directory = Path(p.get("Dir", "/nonexistent"))
        for name in p.get("GoFiles",[]) + p.get("CgoFiles",[]):
            if (directory/name).resolve() == changed.resolve():
                owners.add(identity)
        # -deps also reports dormant test filenames for dependencies and the
        # standard library. Only the root module's packages are execution units.
        if p.get("Module",{}).get("Main") and (p.get("TestGoFiles") or p.get("XTestGoFiles")):
            group = p.get("ForTest") or identity
            groups[group] = directory
    selected = []
    for group, directory in groups.items():
        roots = [identity for identity,p in nodes.items() if Path(p.get("Dir","/nonexistent")) == directory and (p.get("ForTest") == group or identity == group)]
        todo = roots[:]
        visited = set()
        while todo:
            identity = todo.pop()
            if identity in visited:
                continue
            visited.add(identity)
            p = nodes.get(identity)
            if p is None:
                raise ValueError(f"incomplete independent package graph: {identity}")
            todo.extend(p.get("ImportMap",{}).get(i,i) for i in p.get("Imports",[]))
        if visited & owners:
            selected.append(group)
    return {"execution_packages":sorted(groups),"expected_selected":sorted(selected),"changed_owner_ids":sorted(owners)}


def failing_packages(path):
    return sorted({event["Package"] for event in json_stream(path) if event.get("Action") == "fail" and event.get("Package")})


def replace_once(path, before, after):
    if before is None:
        if path.exists():
            raise ValueError(f"new control file already exists: {path}")
        path.write_text(after)
        return
    source = path.read_text()
    if source.count(before) != 1:
        raise ValueError(f"mutation source drift: expected one occurrence in {path}")
    if after is None:
        path.unlink()
    else:
        path.write_text(source.replace(before,after,1))


def evaluate_case(case, benchmark_root, output_dir, native, cli, timeout, go="go"):
    artifacts = output_dir / case.name
    artifacts.mkdir(parents=True,exist_ok=False)
    result = {"case":case.name,"project":case.project,"success":False,"artifact_directory":str(artifacts),"omitted_failing_binaries":None}
    try:
        source = benchmark_root / case.project
        if not source.is_dir():
            raise ValueError(f"missing benchmark project: {source}")
        root = artifacts / "module"
        git(benchmark_root,"clone","--no-hardlinks","--no-checkout",str(source),str(root))
        git(root,"checkout","--detach",case.baseline)
        if case.augment == "sort":
            replace_once(root/"internal/services/geo/geo.go","return candidates[i].dist < candidates[j].dist","return tiaNearer(candidates[i].dist, candidates[j].dist)")
            (root/case.source).write_text("package geo\nfunc tiaNearer(a, b float64) bool {\n return a < b\n}\n")
            commit(root,"TIA sort callback baseline augmentation")
        elif case.augment == "init":
            (root/case.source).write_text("package profile\nfunc init() {\n _ = 1\n}\n")
            commit(root,"TIA explicit initialization baseline control")
        baseline = git(root,"rev-parse","HEAD")
        result.update(original_baseline=case.baseline,baseline=baseline)
        context = driver_context(go,root,artifacts,timeout)
        for label, args in [("go-list",[go,"list","-mod=readonly","-deps","-test","-json","./..."]),("go-list-ordinary",[go,"list","-mod=readonly","-json","./..."])]:
            if command(args,root,artifacts,label,timeout,context):
                diagnostic = (artifacts/f"{label}.stderr").read_text()
                if "operation not permitted" in diagnostic.lower() or "permission denied" in diagnostic.lower():
                    raise RuntimeError(f"Go build/module cache access denied; check sandbox/filesystem permissions; see {label}.stderr. No dependencies were downloaded.")
                raise RuntimeError(f"offline Go package loading failed; inspect {label}.stderr first. If dependencies are missing, prepare them explicitly: cd {shlex.quote(str(source))} && go mod download")
        baseline_status = command([go,"test",*context["build_flags"],"-count=1","-json","./..."],root,artifacts,"baseline-test",timeout,context)
        if baseline_status or failing_packages(artifacts/"baseline-test.stdout"):
            raise RuntimeError("baseline tests did not pass; cannot grade this mutant")
        if git(root,"status","--porcelain"):
            raise RuntimeError("baseline preparation changed module files; dependencies are not ready")
        replace_once(root/case.source,case.before,case.after)
        result["candidate"] = commit(root,"TIA package-policy mutation "+case.name)
        (artifacts/"mutation.patch").write_text(git(root,"diff",baseline,"HEAD","--"))
        analysis_native = native
        if case.augment == "sort":
            shim = artifacts/"metadata-only-native.sh"
            shim.write_text("#!/bin/sh\nif [ \"$1\" = --mode ]; then exec "+shlex.quote(str(native))+" \"$@\"; fi\necho 'graph disabled for callback package-only control' >&2\nexit 1\n")
            shim.chmod(0o755)
            analysis_native = shim
        if command([cli,"--project-dir",root,"--baseline-revision",baseline,"--go-callgraph-binary",analysis_native,"--selection-policy","packages","--test-scope","linked-binary","--analysis-timeout-seconds",str(timeout),"--output",artifacts/"report.json"],root,artifacts,"analysis",timeout*4+60):
            raise RuntimeError("analyzer rejected control; see analysis.stderr")
        report = json.loads((artifacts/"report.json").read_text())
        recorded = report.get("effective_build_context") or context
        if recorded != context:
            raise ValueError("analysis/execution effective build context drift")
        mutant_status = command([go,"test",*recorded["build_flags"],"-count=1","-json","./..."],root,artifacts,"mutant-test",timeout,recorded)
        failures = failing_packages(artifacts/"mutant-test.stdout")
        if mutant_status and not failures:
            raise RuntimeError("mutant test process failed without package failure events; control is inconclusive")
        arguments = execution_arguments(report,root)
        excluded = sorted(p["package"]["import_path"] for p in report["unselected_execution_packages"])
        omitted = sorted(set(failures) & set(excluded))
        complete = report["execution_inventory_complete"]
        result.update(execution_inventory_complete=complete,module_run_required=report["selection_mode"]=="conservative_all",execution_arguments=arguments,selected_binaries=len(report["selected_execution_packages"]) if complete else None,excluded_binaries=len(excluded) if complete else None,selected_tests=len(report["selected_tests"]),excluded_tests=len(report["unselected_tests"]),observed_failing_binaries=failures,omitted_failing_binaries=len(omitted),omitted_binary_names=omitted,selection_mode=report["selection_mode"],diagnostics=report.get("diagnostics",[]))
        selected_status = run_selected_tests(arguments,root,recorded,artifacts,timeout,go=go)
        selected_failures = set(failing_packages(artifacts/"selected-test.stdout")) if arguments else set()
        expected_failures = set(failures) if arguments == ["./..."] else set(failures) & {p["package"]["import_path"] for p in report["selected_execution_packages"]}
        if selected_failures != expected_failures or selected_status and not selected_failures:
            raise RuntimeError("selected test execution disagrees with full-module observation")
        if case.expected_mode != report["selection_mode"]:
            raise ValueError(f"expected {case.expected_mode}; got {report['selection_mode']}; inspect report diagnostics")
        if report["selection_mode"] == "conservative_packages":
            observation = package_observation(json_stream(artifacts/"go-list.stdout"),root/case.source)
            save(artifacts/"package-observation.json",observation)
            actual = sorted(p["package"]["import_path"] for p in report["selected_execution_packages"])
            if actual != observation["expected_selected"] or sorted(p["import_path"] for p in report["discovered_execution_packages"]) != observation["execution_packages"]:
                raise ValueError("package selection disagrees with independent Go-list observation")
            if case.expected_packages is not None and actual != sorted(case.expected_packages):
                raise ValueError(f"acceptance target mismatch: selected {actual}")
            if case.project == "go-micro-services" and case.expected_packages and result["excluded_binaries"] == 0:
                raise ValueError("service edit produced no execution-binary reduction")
            if case.augment == "sort" and report["graph_suggested_tests"]:
                raise ValueError("callback control must demonstrate selection without graph suggestions")
        elif report["run_all_tests"] is not True:
            raise ValueError("broad change did not require whole-module execution")
        if omitted:
            raise ValueError("observed failing binaries were excluded: "+", ".join(omitted))
        result["success"] = True
    except (ValueError,RuntimeError,OSError,subprocess.SubprocessError) as error:
        result["error"] = str(error)
    save(artifacts/"result.json",result)
    print(f"{case.name}: {'PASS' if result['success'] else 'FAIL'} selected={result.get('selected_binaries','?')} excluded={result.get('excluded_binaries','?')} omitted_failures={result['omitted_failing_binaries']}",flush=True)
    return result


MICRO = "f80124996af7180bb28ad3e290a84e941980a45f"
COFFEE = "22d33190a0cf81630878e47bafae2de7c0abb783"
SERVICE = "github.com/harlow/go-micro-services/internal/services/"
SHARED = "github.com/thangchung/go-coffeeshop/internal/pkg/shared_kernel"


def cases():
    def micro(name,path,before,after,service=None,mode="conservative_packages",augment=None):
        return Case(name,"go-micro-services",MICRO,path,before,after,(SERVICE+service,) if service else None,mode,augment)
    return [
        micro("profile","internal/services/profile/profile.go","return s.profiles[id]","return nil","profile"),
        micro("geo-distance","internal/services/geo/geo.go","earthRadiusKm * 2 *","earthRadiusKm * 200 *","geo"),
        micro("rate-results","internal/services/rate/rate.go","res.RatePlans = append(res.RatePlans, s.rateTable[stay])","res.RatePlans = nil","rate"),
        micro("frontend-date","internal/services/frontend/frontend.go",'const dateFmt = "2006-01-02"','const dateFmt = "2006/01/02"',"frontend"),
        micro("frontend-status","internal/services/frontend/frontend.go","w.WriteHeader(status)","w.WriteHeader(http.StatusBadRequest)","frontend"),
        micro("sort-callback","internal/services/geo/tia_callback_control.go","return a < b","return a > b","geo",augment="sort"),
        Case("coffeeshop-id","go-coffeeshop",COFFEE,"internal/pkg/shared_kernel/entity.go","return uuid.New()","return uuid.Nil",(SHARED,)),
        Case("coffeeshop-uncovered-helper","go-coffeeshop",COFFEE,"internal/pkg/shared_kernel/aggregate_root.go","ar.domainEvents = append(ar.domainEvents, e)","ar.domainEvents = nil",(SHARED,)),
        micro("configuration-fallback","tia-control-config.yml",None,"changed: true\n",mode="conservative_all"),
        micro("signature-fallback","internal/services/profile/profile.go","getProfile(id string)","getProfile(id string, ignored ...bool)",mode="conservative_all"),
        micro("deletion-fallback","internal/services/profile/profile_test.go","func TestGetProfile",None,mode="conservative_all"),
        micro("initialization-fallback","internal/services/profile/tia_init_control.go","_ = 1","_ = 2",mode="conservative_all",augment="init"),
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--benchmark-root",type=Path,required=True)
    parser.add_argument("--output-dir",type=Path,required=True)
    parser.add_argument("--go-callgraph-binary",type=Path,required=True)
    parser.add_argument("--go-test-impact-binary",type=Path,required=True)
    parser.add_argument("--analysis-timeout-seconds",type=int,default=300)
    args = parser.parse_args()
    args.output_dir = args.output_dir.resolve()
    args.output_dir.mkdir(parents=True,exist_ok=True)
    if any(args.output_dir.iterdir()):
        parser.error("output directory must be empty; existing artifacts are never overwritten")
    summary = {"scope":"linked-binary","soundness_claim":"empirical controls only, not a general soundness proof","results":[],"success":False}
    try:
        if args.analysis_timeout_seconds <= 0:
            raise ValueError("positive timeout required")
        root = args.benchmark_root.resolve()
        for name in ["go-micro-services","go-coffeeshop"]:
            if not (root/name).is_dir():
                raise ValueError(f"missing benchmark project: {root/name}")
        for binary in [args.go_callgraph_binary,args.go_test_impact_binary]:
            if not binary.is_file() or not os.access(binary,os.X_OK):
                raise ValueError(f"prebuilt executable unavailable: {binary}")
        for case in cases():
            summary["results"].append(evaluate_case(case,root,args.output_dir,args.go_callgraph_binary.resolve(),args.go_test_impact_binary.resolve(),args.analysis_timeout_seconds))
            save(args.output_dir/"summary.json",summary)
        summary["success"] = all(r["success"] for r in summary["results"])
        summary["omitted_failing_binaries"] = sum(r.get("omitted_failing_binaries") or 0 for r in summary["results"])
        summary["inconclusive_cases"] = sum(r.get("omitted_failing_binaries") is None for r in summary["results"])
    except (ValueError,RuntimeError,OSError) as error:
        summary["error"] = str(error)
        print(str(error),file=sys.stderr)
    save(args.output_dir/"summary.json",summary)
    print(f"Results: {args.output_dir/'summary.json'}",flush=True)
    return 0 if summary["success"] else 1


if __name__ == "__main__":
    sys.exit(main())
