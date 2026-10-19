#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
PYTHONDONTWRITEBYTECODE=1 python3 - "$repo_root" <<'PY'
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

repo = Path(sys.argv[1])
sys.path.insert(0, str(repo / "scripts"))
spec = importlib.util.spec_from_file_location("package_bench", repo / "scripts/evaluate-go-package-selection.py")
m = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = m
spec.loader.exec_module(m)

CONTEXT = {"go_version":"go1.26.0","goos":"linux","goarch":"amd64","cgo_enabled":"0","architecture_features":{"GOAMD64":"v1"},"goexperiment":"","build_tags":"","build_flags":["-mod=readonly"],"environment":{"GOOS":"linux","GOARCH":"amd64","CGO_ENABLED":"0","GOAMD64":"v1","GOEXPERIMENT":"","GOENV":"off","GOFLAGS":"","GOWORK":"off","GOTOOLCHAIN":"local","GOPROXY":"off","GOSUMDB":"off"}}

def report():
    packages = [{"import_path":"example/a","relative_directory":"a","roots":["a"]},{"import_path":"example/b","relative_directory":"b","roots":["b"]}]
    return {"schema_version":2,"requested_policy":"packages","declared_test_scope":"linked-binary","selection_mode":"conservative_packages","run_all_tests":False,"execution_inventory_complete":True,"effective_build_context":CONTEXT,"discovered_tests":[],"selected_tests":[],"unselected_tests":[],"graph_suggested_tests":[],"diagnostics":[],"discovered_execution_packages":packages,"selected_execution_packages":[{"package":packages[0],"argument":"./a"}],"unselected_execution_packages":[{"package":packages[1],"argument":"./b"}]}

def executable(path, body):
    path.write_text("#!/usr/bin/env python3\n" + body)
    path.chmod(0o755)
    return path

class Controls(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="package control spaces ")
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.projects = self.base / "projects"
        self.source = self.projects / "control"
        (self.source / "a").mkdir(parents=True)
        (self.source / "b").mkdir()
        (self.source / "go.mod").write_text("module example\n\ngo 1.26.0\n")
        (self.source / "a/a.go").write_text("package a\nfunc Value() int { return 1 }\n")
        (self.source / "a/a_test.go").write_text("package a\n")
        (self.source / "b/b_test.go").write_text("package b\n")
        def git(*args):
            return subprocess.check_output(["git","-C",str(self.source),"-c","user.name=Control","-c","user.email=control@example.test","-c","commit.gpgsign=false",*args],stderr=subprocess.DEVNULL,text=True).strip()
        git("init","-q"); git("add","."); git("commit","-qm","base")
        self.sha = git("rev-parse","HEAD")
        self.case = m.Case("control","control",self.sha,"a/a.go","return 1","return 2",None)
        self.config = self.base / "config.json"
        self.config.write_text(json.dumps({"report":report(),"fail_package":"example/b"}))
        self.calls = self.base / "go-calls.jsonl"
        self.go = executable(self.base / "go", f'''import json, os, pathlib, sys
cfg=json.loads(pathlib.Path({str(self.config)!r}).read_text())
args=sys.argv[1:]
with open({str(self.calls)!r},'a') as f: f.write(json.dumps({{'args':args,'GOENV':os.environ.get('GOENV'),'GOFLAGS':os.environ.get('GOFLAGS')}})+'\\n')
if args[0]=='env':
 print(json.dumps({{'GOVERSION':'go1.26.0','GOOS':'linux','GOARCH':'amd64','GOHOSTOS':'linux','GOHOSTARCH':'amd64','CGO_ENABLED':'0','GOAMD64':'v1','GOEXPERIMENT':''}}))
elif args[0]=='list':
 if cfg.get('missing_dependencies'): print(cfg.get('load_error','missing dependency offline'),file=sys.stderr); sys.exit(1)
 data=[{{'ImportPath':'example/a','Dir':str(pathlib.Path.cwd()/'a'),'GoFiles':['a.go'],'TestGoFiles':['a_test.go'],'Imports':[]}},{{'ImportPath':'a-test-opaque','ForTest':'example/a','Dir':str(pathlib.Path.cwd()/'a'),'GoFiles':['a.go','a_test.go'],'Imports':[]}},{{'ImportPath':'example/b','Dir':str(pathlib.Path.cwd()/'b'),'GoFiles':[],'TestGoFiles':['b_test.go'],'Imports':[]}},{{'ImportPath':'b-test-opaque','ForTest':'example/b','Dir':str(pathlib.Path.cwd()/'b'),'GoFiles':['b_test.go'],'Imports':[]}}]
 for item in data:
  item['Module']={{'Main':True}}
  print(json.dumps(item))
elif args[0]=='test':
 mutant='return 2' in (pathlib.Path.cwd()/'a/a.go').read_text()
 if not mutant and cfg.get('baseline_failure'): print(json.dumps({{'Action':'fail','Package':'example/a'}})); sys.exit(1)
 if mutant and cfg.get('build_failure'):
  print(json.dumps({{'Action':'build-fail','ImportPath':'example/a'}})); print(json.dumps({{'Action':'fail','Package':'example/a','FailedBuild':'example/a'}})); sys.exit(1)
 if mutant and cfg.get('infrastructure_failure'): print('test invocation failed',file=sys.stderr); sys.exit(2)
 targets=['example/a','example/b'] if './...' in args else [p.replace('./','example/') for p in args if p.startswith('./')]
 failed=False
 for p in targets:
  bad=bool(mutant and (p==cfg.get('fail_package') or (cfg.get('selected_only_failure') and './...' not in args)))
  print(json.dumps({{'Action':'fail' if bad else 'pass','Package':p}}));failed |= bad
 sys.exit(1 if failed else 0)
''')
        self.cli = executable(self.base / "cli", f'''import json,pathlib,sys
args=sys.argv[1:]; out=pathlib.Path(args[args.index('--output')+1]); out.write_text(json.dumps(json.loads(pathlib.Path({str(self.config)!r}).read_text())['report']))
''')
    def evaluate(self):
        return m.evaluate_case(self.case,self.projects,self.base / "results",self.cli,self.cli,5,go=str(self.go))
    def test_excluded_failure_is_a_failed_control_with_preserved_artifacts(self):
        result=self.evaluate()
        self.assertFalse(result['success'])
        self.assertEqual(result['omitted_failing_binaries'],1)
        self.assertEqual(result['observed_failing_binaries'],['example/b'])
        self.assertEqual((self.source/'a/a.go').read_text(),"package a\nfunc Value() int { return 1 }\n")
        self.assertTrue((self.base/'results/control/report.json').is_file())
        self.assertTrue((self.base/'results/control/mutant-test.stdout').is_file())
    def test_complete_selection_executes_exact_packages_with_recorded_context(self):
        cfg=json.loads(self.config.read_text());cfg['fail_package']='example/a';self.config.write_text(json.dumps(cfg))
        result=self.evaluate();self.assertTrue(result['success'],result)
        calls=[json.loads(line) for line in self.calls.read_text().splitlines()]
        self.assertTrue(any(c['args'][0]=='test' and c['args'][-1]=='./a' and c['GOENV']=='off' and c['GOFLAGS']=='' for c in calls))
    def test_empty_selection_skips_selected_test_command(self):
        cfg=json.loads(self.config.read_text());r=cfg['report'];r['unselected_execution_packages']+=r['selected_execution_packages'];r['selected_execution_packages']=[];cfg['fail_package']=None;self.config.write_text(json.dumps(cfg))
        # An unused source is absent from every observed binary dependency.
        # Test the execution consumer independently of the import-oracle gate.
        args=m.execution_arguments(r,self.source);self.assertEqual(args,[])
        out=self.base/'empty';out.mkdir();m.run_selected_tests(args,self.source,CONTEXT,out,5,go=str(self.go))
        self.assertFalse(self.calls.exists())
        self.assertTrue((out/'selected-test.skipped').is_file())
    def test_selected_execution_must_agree_with_full_module_observation(self):
        cfg=json.loads(self.config.read_text());cfg['fail_package']=None;cfg['selected_only_failure']=True;self.config.write_text(json.dumps(cfg))
        result=self.evaluate();self.assertFalse(result['success']);self.assertIn('selected',result['error'])
        self.assertTrue(result['inconclusive'])
    def test_test_process_error_without_failure_events_is_inconclusive_not_success(self):
        cfg=json.loads(self.config.read_text());cfg['infrastructure_failure']=True;self.config.write_text(json.dumps(cfg))
        result=self.evaluate();self.assertFalse(result['success']);self.assertIn('test',result['error'])
    def test_incomplete_or_metapattern_instructions_are_rejected(self):
        r=report();r['execution_inventory_complete']=False
        with self.assertRaises(ValueError):m.execution_arguments(r,self.source)
        r=report();r['selected_execution_packages'][0]['argument']='./a/...'
        with self.assertRaises(ValueError):m.execution_arguments(r,self.source)
    def test_dependency_test_metadata_is_not_a_local_execution_binary(self):
        records=[{'ImportPath':'stdlib','Dir':'/stdlib','GoFiles':['s.go'],'TestGoFiles':['s_test.go'],'Imports':[]}, {'ImportPath':'external','Dir':'/cache/external','GoFiles':['x.go'],'TestGoFiles':['x_test.go'],'Module':{'Main':False},'Imports':[]}, {'ImportPath':'example/a','Dir':str(self.source/'a'),'GoFiles':['a.go'],'TestGoFiles':['a_test.go'],'Module':{'Main':True},'Imports':[]}]
        observation=m.package_observation(records,self.source/'a/a.go')
        self.assertEqual(observation['execution_packages'],['example/a'])
        self.assertEqual(observation['expected_selected'],['example/a'])
    def test_missing_dependencies_fail_without_downloading(self):
        cfg=json.loads(self.config.read_text());cfg['missing_dependencies']=True;self.config.write_text(json.dumps(cfg))
        result=self.evaluate();self.assertFalse(result['success']);self.assertIn('go mod download',result['error'])
        self.assertTrue((self.base/'results/control/result.json').is_file())
        self.assertFalse(any('download' in json.loads(line)['args'] for line in self.calls.read_text().splitlines()))
    def test_missing_project_runner_fails_and_keeps_output(self):
        out=self.base/'missing output'
        env=dict(os.environ,GO_BENCHMARK_ROOT=str(self.base/'missing'),TIA_BENCHMARK_OUTPUT_DIR=str(out),GO_CALLGRAPH_BINARY=str(self.cli),GO_TEST_IMPACT_BINARY=str(self.cli))
        done=subprocess.run(['bash',str(repo/'scripts/run-go-package-selection-benchmarks.sh')],env=env,capture_output=True,text=True)
        self.assertNotEqual(done.returncode,0);self.assertTrue((out/'summary.json').is_file())
    def test_cache_access_denial_does_not_recommend_a_dependency_download(self):
        cfg=json.loads(self.config.read_text());cfg.update(missing_dependencies=True,load_error='open Go build cache: operation not permitted');self.config.write_text(json.dumps(cfg))
        result=self.evaluate();self.assertFalse(result['success']);self.assertIn('cache access',result['error']);self.assertNotIn('go mod download',result['error'])
    def test_campaign_readiness_preserves_unsupported_projects_and_pinned_baselines(self):
        import go_package_campaign as campaign
        nested=self.projects/'nested';(nested/'service').mkdir(parents=True)
        (nested/'service/go.mod').write_text('module nested\n')
        m.git(nested,'init','-q');m.commit(nested,'nested module')
        untested=self.projects/'untested';untested.mkdir()
        (untested/'go.mod').write_text('module untested\n');(untested/'u.go').write_text('package untested\n')
        m.git(untested,'init','-q');m.commit(untested,'no tests')
        out=self.base/'readiness';out.mkdir()
        results=campaign.inspect_projects(m,self.projects,out,self.cli,5,go=str(self.go))
        by_name={r['project']:r for r in results}
        self.assertEqual(by_name['nested']['status'],'unsupported_layout')
        self.assertEqual(by_name['untested']['status'],'no_tests')
        self.assertEqual(by_name['control']['revision'],self.sha)
        self.assertFalse(by_name['control']['status']=='ready') # Fake CLI is not package evidence.
        self.assertTrue((out/'control/result.json').is_file())
        self.assertFalse((self.source/'go.sum').exists())
    def test_campaign_summary_does_not_turn_fallback_or_survivors_into_detections(self):
        import go_package_campaign as campaign
        rows=[dict(project='a',success=True,selection_mode='conservative_packages',selected_binaries=1,excluded_binaries=4,observed_failing_binaries=['a/p'],omitted_failing_binaries=0,durations_seconds={'analysis':2,'mutant-test':10,'selected-test':3}),
              dict(project='a',success=True,selection_mode='conservative_packages',selected_binaries=1,excluded_binaries=0,observed_failing_binaries=[],omitted_failing_binaries=0,durations_seconds={'analysis':2,'mutant-test':1,'selected-test':1}),
              dict(project='b',success=True,selection_mode='conservative_all',selected_binaries=None,excluded_binaries=None,observed_failing_binaries=['b/p'],omitted_failing_binaries=0,durations_seconds={}),
              dict(project='b',success=False,omitted_failing_binaries=None,error='baseline failure')]
        summary=campaign.summarize(rows, [{'project':'a','status':'ready'},{'project':'b','status':'baseline_failed'}])
        self.assertEqual(summary['package_selection_cases'],2)
        self.assertEqual(summary['reduced_cases'],1)
        self.assertEqual(summary['surviving_mutants'],1)
        self.assertEqual(summary['failure_producing_package_cases'],1)
        self.assertEqual(summary['fallback_cases'],1)
        self.assertEqual(summary['inconclusive_cases'],1)
        self.assertEqual(summary['timed_cases'][0]['analysis_plus_selected_seconds'],5)
        self.assertEqual(summary['timed_cases'][0]['net_saving_seconds'],5)
        self.assertFalse(summary['coverage_target_met'])
    def test_expanded_manifest_is_unique_body_mutations_and_preserves_original_controls(self):
        import go_package_campaign as campaign
        cases=campaign.expanded_cases(m)
        bodies=[c for c in cases if c.expected_mode=='conservative_packages']
        self.assertGreaterEqual(len(bodies),30)
        self.assertEqual(len({c.name for c in cases}),len(cases))
        self.assertEqual(len({(c.project,c.source,c.before,c.after) for c in bodies}),len(bodies))
        self.assertTrue(all(c.before and c.after and c.before!=c.after for c in bodies))
        self.assertTrue(set(c.name for c in m.cases()) <= set(c.name for c in cases))
    def test_compile_error_is_invalid_mutant_not_a_captured_test_failure(self):
        cfg=json.loads(self.config.read_text());cfg['build_failure']=True;self.config.write_text(json.dumps(cfg))
        result=self.evaluate()
        self.assertFalse(result['success'])
        self.assertEqual(result['mutation_outcome'],'invalid_mutant')
        self.assertIsNone(result['omitted_failing_binaries'])
    def test_readiness_baseline_failure_and_blockers_are_not_eligible(self):
        import go_package_campaign as campaign
        native=executable(self.base/'metadata', "import json\nprint(json.dumps({'schema_version':1,'provider_id':'go-native','status':'ok','complete':True,'execution_packages':[{'import_path':'example/a'}],'blockers':[]}))\n")
        out=self.base/'ready';out.mkdir()
        rows=campaign.inspect_projects(m,self.projects,out,native,5,go=str(self.go))
        self.assertEqual(rows[0]['status'],'ready')
        cfg=json.loads(self.config.read_text());cfg['baseline_failure']=True;self.config.write_text(json.dumps(cfg))
        out=self.base/'bad-baseline';out.mkdir()
        rows=campaign.inspect_projects(m,self.projects,out,native,5,go=str(self.go))
        self.assertEqual(rows[0]['status'],'baseline_failed')
        cfg['baseline_failure']=False;self.config.write_text(json.dumps(cfg))
        native=executable(native, "import json\nprint(json.dumps({'schema_version':1,'provider_id':'go-native','status':'ok','complete':True,'execution_packages':[{'import_path':'example/a'}],'blockers':[{'code':'process_execution'}]}))\n")
        out=self.base/'blocked';out.mkdir()
        rows=campaign.inspect_projects(m,self.projects,out,native,5,go=str(self.go))
        self.assertEqual(rows[0]['status'],'package_evidence_unavailable')
    def test_campaign_keeps_counterexamples_in_capture_denominators(self):
        import go_package_campaign as campaign
        common=dict(project='a',selection_mode='conservative_packages',selected_binaries=1,excluded_binaries=4,observed_failing_binaries=['a/p'],durations_seconds={'analysis':2,'mutant-test':10,'selected-test':3})
        rows=[dict(common,case='captured',success=True,omitted_failing_binaries=0),dict(common,case='omitted',success=False,omitted_failing_binaries=1)]
        summary=campaign.summarize(rows,[])
        self.assertEqual(summary['failure_producing_package_cases'],2)
        self.assertEqual(summary['package_selection_cases'],2)
        self.assertEqual(summary['captured_failure_cases'],1)
        self.assertEqual(len(summary['timed_cases']),2)
        self.assertFalse(summary['success'])
    def test_readiness_uses_replay_revision_not_unrelated_current_head(self):
        import go_package_campaign as campaign
        (self.source/'nested').mkdir();(self.source/'nested/go.mod').write_text('module nested\n')
        head=m.commit(self.source,'unsupported new head')
        native=executable(self.base/'metadata', "import json\nprint(json.dumps({'schema_version':1,'provider_id':'go-native','status':'ok','complete':True,'execution_packages':[{'import_path':'example/a'}],'blockers':[]}))\n")
        out=self.base/'pinned-readiness';out.mkdir()
        rows=campaign.inspect_projects(m,self.projects,out,native,5,go=str(self.go),revisions={'control':[self.sha]})
        self.assertEqual(rows[0]['status'],'ready')
        self.assertEqual(rows[0]['revision'],self.sha)
        self.assertEqual(rows[0]['checkout_head'],head)
        self.assertEqual(m.git(self.source,'rev-parse','HEAD'),head)
    def test_breadth_and_mutation_shortfalls_are_reported_independently(self):
        import go_package_campaign as campaign
        rows=[dict(project=p,success=True,selection_mode='conservative_packages',selected_binaries=1,excluded_binaries=0,observed_failing_binaries=[],omitted_failing_binaries=0) for p in ['a','b','c']]
        summary=campaign.summarize(rows,[])
        self.assertEqual(len(summary['limitations']),1)
        self.assertIn('mutation',summary['limitations'][0])
        self.assertNotIn('project',summary['limitations'][0])
    def test_observed_survivors_include_unexpected_fallback_cases(self):
        import go_package_campaign as campaign
        rows=[dict(project='a',success=False,inconclusive=False,selection_mode='conservative_all',selected_binaries=5,excluded_binaries=0,observed_failing_binaries=[],omitted_failing_binaries=0)]
        summary=campaign.summarize(rows,[])
        self.assertEqual(summary['surviving_mutants'],1)
        self.assertEqual(summary['fallback_cases'],1)
        self.assertFalse(summary['success'])

unittest.main(argv=[sys.argv[0]],verbosity=2)
PY
