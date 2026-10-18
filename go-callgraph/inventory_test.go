package main

import (
	"bytes"
	"context"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func testModule(t *testing.T, files map[string]string) string {
	t.Helper()
	root := t.TempDir()
	for name, code := range files {
		path := filepath.Join(root, name)
		if err := os.MkdirAll(filepath.Dir(path), 0700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(code), 0600); err != nil {
			t.Fatal(err)
		}
	}
	return root
}
func TestLineDirectivesKeepPhysicalInventoryAndGraphOwnership(t *testing.T) {
	root := testModule(t, map[string]string{
		"go.mod":    "module lines.test/c\n\ngo 1.26.0\n",
		"s.go":      "package c\n//line /outside/generated.go:1000\nfunc Changed() int { return 1 }\n",
		"s_test.go": "//line /outside/generated.go:2000\npackage c\nimport \"testing\"\n//line /outside/generated.go:3000\nfunc TestChanged(t *testing.T) { Changed() }\n",
	})
	root, err := filepath.EvalSymlinks(root)
	if err != nil {
		t.Fatal(err)
	}
	out := Discover(context.Background(), Options{SourceDir: root})
	if out.Status != "ok" || !out.TestInventoryComplete || len(out.Tests) != 1 {
		t.Fatalf("line directive hid real test: %+v", out)
	}
	ref := out.Tests[0].Method
	if ref.SourcePath != filepath.Join(root, "s_test.go") || ref.SourceLine != 5 {
		t.Fatalf("nonphysical test identity: %+v", ref)
	}
	if len(out.Declarations) != 2 {
		t.Fatalf("lost physical declarations: %+v", out.Declarations)
	}
	for _, d := range out.Declarations {
		if d.Syntax.StartLine != 3 && d.Syntax.StartLine != 5 {
			t.Fatalf("nonphysical declaration span: %+v", d)
		}
	}
	graph := Analyze(context.Background(), Options{SourceDir: root, Targets: []string{"lines.test/c#Changed"}})
	if graph.Status != "ok" || len(graph.Tests) != 1 || len(graph.Reachability) != 1 || len(graph.Reachability[0].Entrypoints) != 1 || graph.Reachability[0].Method.SourceLine != 3 || graph.Tests[0] != ref {
		t.Fatalf("line directives broke graph identities: %+v", graph)
	}
}

func TestSavedGoConfigurationCannotChangeReportedScope(t *testing.T) {
	config := filepath.Join(t.TempDir(), "goenv")
	if err := os.WriteFile(config, []byte("GOFLAGS=-tags=hidden\nGOOS=plan9\nGOARCH=386\n"), 0600); err != nil {
		t.Fatal(err)
	}
	t.Setenv("GOENV", config)
	t.Setenv("GOFLAGS", "")
	t.Setenv("GOOS", "")
	t.Setenv("GOARCH", "")
	root := testModule(t, map[string]string{
		"go.mod":         "module saved.test/c\n\ngo 1.26.0\n",
		"s.go":           "package c\nfunc Changed() {}\n",
		"host_test.go":   "//go:build " + runtime.GOOS + " && " + runtime.GOARCH + "\n\npackage c\nimport \"testing\"\nfunc TestHost(t *testing.T) { Changed() }\n",
		"hidden_test.go": "//go:build hidden\n\npackage c\nimport \"testing\"\nfunc TestHidden(t *testing.T) { Changed() }\n",
	})
	for _, tags := range []string{"", "hidden"} {
		want := 1
		if tags != "" {
			want = 2
		}
		inventory := Discover(context.Background(), Options{SourceDir: root, Tags: tags})
		graph := Analyze(context.Background(), Options{SourceDir: root, Tags: tags, Targets: []string{"saved.test/c#Changed"}})
		foundHost := false
		for _, test := range inventory.Tests {
			foundHost = foundHost || test.Method.MemberName == "TestHost"
		}
		if inventory.Status != "ok" || !inventory.TestInventoryComplete || len(inventory.Tests) != want || graph.Status != "ok" || len(graph.Tests) != want || inventory.Scope.GOOS != runtime.GOOS || inventory.Scope.GOARCH != runtime.GOARCH || inventory.Scope.BuildTags != tags || graph.Scope.BuildTags != tags {
			t.Fatalf("saved configuration changed reported scope: inventory=%+v graph=%+v", inventory, graph)
		}
		if !foundHost || tags == "" && graph.Tests[0].MemberName != "TestHost" {
			t.Fatalf("saved configuration changed active tests: inventory=%+v graph=%+v", inventory, graph)
		}
	}
}

func TestInventoryFindsSevenTestsWithoutBuildingGraph(t *testing.T) {
	var progress bytes.Buffer
	out := Discover(context.Background(), Options{SourceDir: "testdata/control", Progress: &progress})
	if out.Status != "ok" || !out.TestInventoryComplete || len(out.Tests) != 7 {
		t.Fatalf("inventory: %+v", out)
	}
	for _, test := range out.Tests {
		if test.Method.MemberName == "TestExternal" && (test.ExecutionPackage != "example.com/tia" || test.Method.DeclaringType != "example.com/tia_test") {
			t.Fatalf("external identity: %+v", test)
		}
	}
	if strings.Contains(progress.String(), "graph_ready") {
		t.Fatal("inventory built a graph")
	}
}
func TestInventoryRetainsTestsWithTestMain(t *testing.T) {
	root := testModule(t, map[string]string{"go.mod": "module setup.test/c\n\ngo 1.26.0\n", "s_test.go": "package c; import \"testing\"; func TestMain(m *testing.M) { m.Run() }; func TestUnit(t *testing.T) {}"})
	out := Discover(context.Background(), Options{SourceDir: root})
	if out.Status != "ok" || len(out.Tests) != 1 || !out.TestInventoryComplete {
		t.Fatalf("lost tests: %+v", out)
	}
	if len(out.Diagnostics) == 0 {
		t.Fatal("setup limitation hidden")
	}
}
func TestInventoryHonorsTagsAndAliases(t *testing.T) {
	root := testModule(t, map[string]string{"go.mod": "module tags.test/c\n\ngo 1.26.0\n", "s.go": "package c", "tag_test.go": "//go:build control\n\npackage c; import \"testing\"; type T = testing.T; func TestAlias(t *T) {}"})
	plain := Discover(context.Background(), Options{SourceDir: root})
	tagged := Discover(context.Background(), Options{SourceDir: root, Tags: "control"})
	if len(plain.Tests) != 0 || len(tagged.Tests) != 1 || tagged.Scope.BuildTags != "control" {
		t.Fatalf("tags: %+v %+v", plain, tagged)
	}
}
func TestInventoryRecordsFilesWithoutFunctions(t *testing.T) {
	root := testModule(t, map[string]string{"go.mod": "module globals.test/c\n\ngo 1.26.0\n", "s.go": "package c; var Value = 1"})
	out := Discover(context.Background(), Options{SourceDir: root})
	if out.Status != "ok" || len(out.SourceFiles) != 1 || !strings.HasSuffix(out.SourceFiles[0], "s.go") {
		t.Fatalf("lost globals file: %+v", out)
	}
}
func TestDiscoveryDoesNotRewriteModuleFiles(t *testing.T) {
	for _, mod := range []string{"module missing.test/c\n\ngo 1.26.0\n\nrequire absent.invalid/lib v1.0.0\n", "module local.test/c\n\ngo 1.26.0\n"} {
		root := testModule(t, map[string]string{"go.mod": mod, "s.go": "package c; import _ \"absent.invalid/lib\""})
		out := Discover(context.Background(), Options{SourceDir: root})
		if out.Status != "failed" || out.TestInventoryComplete {
			t.Fatalf("hidden load failure: %+v", out)
		}
		got, _ := os.ReadFile(filepath.Join(root, "go.mod"))
		if string(got) != mod {
			t.Fatal("go.mod modified")
		}
		if _, err := os.Stat(filepath.Join(root, "go.sum")); !os.IsNotExist(err) {
			t.Fatal("go.sum created")
		}
	}
}
