package main

import (
	"context"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"testing"
)

func names(refs []MethodRef) []string {
	result := []string{}
	for _, ref := range refs {
		result = append(result, ref.MemberName)
	}
	sort.Strings(result)
	return result
}

func TestNativeReachabilityAcrossTestVariantsAndPackages(t *testing.T) {
	result := Analyze(context.Background(), Options{SourceDir: "testdata/control", Targets: []string{
		"example.com/tia#Changed", "example.com/tia#NeverCalled", "example.com/tia#Initialize",
	}})
	if result.Status != "ok" {
		t.Fatalf("analysis failed: %+v", result)
	}
	if len(result.Tests) != 7 {
		t.Fatalf("duplicate or missing test variants: %+v", result.Tests)
	}
	records := map[string]Reachability{}
	for _, record := range result.Reachability {
		records[record.Method.MemberName] = record
	}
	want := []string{"TestCrossPackage", "TestDirect", "TestExternal", "TestInterface", "TestTransitive"}
	if got := names(records["Changed"].Entrypoints); !reflect.DeepEqual(got, want) {
		t.Fatalf("Changed: %v, want %v", got, want)
	}
	if record, ok := records["NeverCalled"]; !ok || len(record.Entrypoints) != 0 {
		t.Fatalf("unreached target must have an explicit empty record: %+v", record)
	}
	initTests := names(records["Initialize"].Entrypoints)
	if len(initTests) != 6 {
		t.Fatalf("package initialization must affect all importing tests, not the independent package: %v", initTests)
	}
	for _, ref := range result.Tests {
		if ref.Language != "go" || ref.SourcePath == "" || ref.SourceLine == 0 {
			t.Fatalf("missing native source identity: %+v", ref)
		}
	}
	if result.Scope.DependencyBodies != "opaque" {
		t.Fatal("partial dependency scope must be explicit")
	}
}

func TestTypeErrorsAndMissingTargetsFailExplicitly(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "go.mod"), []byte("module invalid.test/control\n\ngo 1.23.0\n"), 0600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "bad.go"), []byte("package control; func Broken() { undefined() }"), 0600); err != nil {
		t.Fatal(err)
	}
	result := Analyze(context.Background(), Options{SourceDir: dir})
	if result.Status != "failed" || len(result.Diagnostics) == 0 {
		t.Fatalf("type error was hidden: %+v", result)
	}
	result = Analyze(context.Background(), Options{SourceDir: "testdata/control", Targets: []string{"example.com/tia#Missing"}})
	if result.Status != "failed" {
		t.Fatalf("unmatched target became empty success: %+v", result)
	}
}

func TestTestMainIsNotSilentlyIgnored(t *testing.T) {
	dir := t.TempDir()
	for name, contents := range map[string]string{
		"go.mod":          "module setup.test/control\n\ngo 1.26.0\n",
		"service.go":      "package control; func Changed() {}",
		"service_test.go": "package control; import (\"testing\"; \"os\"); func TestMain(m *testing.M) { Changed(); os.Exit(m.Run()) }; func TestUnit(t *testing.T) {}",
	} {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(contents), 0600); err != nil {
			t.Fatal(err)
		}
	}
	out := Analyze(context.Background(), Options{SourceDir: dir, Targets: []string{"setup.test/control#Changed"}})
	if out.Status != "failed" || len(out.Diagnostics) == 0 {
		t.Fatalf("unsupported setup was hidden: %+v", out)
	}
}

// Internal and external tests execute in one binary: the external package's
// initialization affects internal tests even when their bodies never call it.
func TestExternalPackageInitializationAffectsSiblingInternalTests(t *testing.T) {
	dir := t.TempDir()
	for name, contents := range map[string]string{
		"go.mod":           "module shared.test/control\n\ngo 1.26.0\n",
		"service.go":       "package control; func Changed() int { return 1 }",
		"service_test.go":  "package control; import \"testing\"; func TestInternal(t *testing.T) {}",
		"external_test.go": "package control_test; import (\"testing\"; c \"shared.test/control\"); var seed = c.Changed(); func TestExternal(t *testing.T) {}",
	} {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(contents), 0600); err != nil {
			t.Fatal(err)
		}
	}
	out := Analyze(context.Background(), Options{SourceDir: dir, Targets: []string{"shared.test/control#Changed"}})
	if out.Status != "ok" || len(out.Reachability) != 1 {
		t.Fatalf("analysis failed: %+v", out)
	}
	if got := names(out.Reachability[0].Entrypoints); !reflect.DeepEqual(got, []string{"TestExternal", "TestInternal"}) {
		t.Fatalf("shared binary initialization missed tests: %v", got)
	}
}

func TestTestingTypeAliasIsDiscovered(t *testing.T) {
	dir := t.TempDir()
	for name, contents := range map[string]string{
		"go.mod":          "module alias.test/control\n\ngo 1.26.0\n",
		"service.go":      "package control; func Changed() {}",
		"service_test.go": "package control; import \"testing\"; type T = testing.T; func TestAlias(t *T) { Changed() }",
	} {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(contents), 0600); err != nil {
			t.Fatal(err)
		}
	}
	out := Analyze(context.Background(), Options{SourceDir: dir, Targets: []string{"alias.test/control#Changed"}})
	if out.Status != "ok" || len(out.Tests) != 1 || len(out.Reachability[0].Entrypoints) != 1 {
		t.Fatalf("valid testing.T alias was omitted: %+v", out)
	}
}

func TestRepeatedTargetsPreserveTraversalSeeds(t *testing.T) {
	out := Analyze(context.Background(), Options{SourceDir: "testdata/control", Targets: []string{"example.com/tia#Changed", "example.com/tia#Changed", "example.com/tia#Changed"}})
	if out.Status != "ok" || len(out.Reachability) != 3 {
		t.Fatalf("analysis failed: %+v", out)
	}
	for _, record := range out.Reachability {
		if got := names(record.Entrypoints); !reflect.DeepEqual(got, []string{"TestCrossPackage", "TestDirect", "TestExternal", "TestInterface", "TestTransitive"}) {
			t.Fatalf("traversal mutated stored seeds: %v", got)
		}
	}
}
