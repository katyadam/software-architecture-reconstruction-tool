package main

import (
	"context"
	"fmt"
	"golang.org/x/tools/go/packages"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
	"testing"
)

// These tests catch truncated dependencies, lost binary roots, and mistaken
// source ownership. Expected identities come from the fixture, not ID syntax.
func packageFixture(t *testing.T) string {
	t.Helper()
	return testModule(t, map[string]string{
		"go.mod":               "module packages.test/control\n\ngo 1.26.0\n",
		"helper/helper.go":     "package helper\nfunc Value() int { return 1 }\n",
		"app/app.go":           "package app\nimport \"packages.test/control/helper\"\nvar V = helper.Value()\nfunc Value() int { return V }\n",
		"app/app_test.go":      "package app\nimport \"testing\"\nfunc TestInside(t *testing.T) { Value() }\nfunc testHelper() {}\n",
		"app/external_test.go": "package app_test\nimport (\"testing\"; \"packages.test/control/app\")\nfunc TestOutside(t *testing.T) { app.Value() }\n",
	})
}

func packageEvidence(t *testing.T, root string) PackageInventoryOutcome {
	t.Helper()
	out := DiscoverPackages(context.Background(), Options{SourceDir: root})
	if out.Status != "ok" || !out.Complete || out.SchemaVersion != 1 || out.ProviderID != "go-native" {
		t.Fatalf("package evidence failed: status=%s diagnostics=%v", out.Status, out.Diagnostics)
	}
	return out
}

func TestPackageInventoryDirectTransitiveAndExternalBridge(t *testing.T) {
	root := testModule(t, map[string]string{
		"go.mod":             "module bridge.test/main\n\ngo 1.26.0\nrequire bridge.test/external v0.0.0\nreplace bridge.test/external => ./external\n",
		"external/go.mod":    "module bridge.test/external\n\ngo 1.26.0\n",
		"external/bridge.go": "package external\nimport \"bridge.test/main/helper\"\nfunc Value() int { return helper.Value() }\n",
		"helper/helper.go":   "package helper\nfunc Value() int { return 1 }\n",
		"app/app.go":         "package app\nimport \"bridge.test/external\"\nfunc Value() int { return external.Value() }\n",
		"app/app_test.go":    "package app\nimport \"testing\"\nfunc TestValue(t *testing.T) { Value() }\n",
	})
	out := packageEvidence(t, root)
	nodes := map[string]PackageNode{}
	for _, p := range out.Packages {
		nodes[p.ID] = p
	}
	found := false
	for _, p := range out.Packages {
		if p.ImportPath != "bridge.test/main/app" {
			continue
		}
		for _, id := range p.Imports {
			dep := nodes[id]
			if dep.ImportPath != "bridge.test/external" {
				continue
			}
			for _, next := range dep.Imports {
				found = found || nodes[next].ImportPath == "bridge.test/main/helper"
			}
		}
	}
	if !found {
		t.Fatal("lost local -> external -> local dependency path")
	}
}

func TestPackageInventoryVariantsAndInit(t *testing.T) {
	out := packageEvidence(t, packageFixture(t))
	if len(out.ExecutionPackages) != 1 || out.ExecutionPackages[0].ImportPath != "packages.test/control/app" || out.ExecutionPackages[0].RelativeDirectory != "app" {
		t.Fatalf("binary grouping: %+v", out.ExecutionPackages)
	}
	prodOwners, internalRoots, externalRoots := 0, 0, 0
	nodes := map[string]PackageNode{}
	for _, p := range out.Packages {
		nodes[p.ID] = p
		for _, f := range p.SourceFiles {
			if strings.HasSuffix(f, "/app/app.go") {
				prodOwners++
			}
		}
		if p.ForTest == "" && p.ImportPath == "packages.test/control/app" && len(p.TestFiles) != 0 {
			t.Fatal("test-only files leaked into production")
		}
		if !slices.Contains(out.ExecutionPackages[0].Roots, p.ID) {
			continue
		}
		if p.ImportPath == "packages.test/control/app" {
			internalRoots++
		}
		if p.ImportPath == "packages.test/control/app_test" {
			externalRoots++
		}
	}
	if prodOwners != 2 || internalRoots != 1 || externalRoots != 1 {
		t.Fatalf("ownership/roots: %d/%d/%d", prodOwners, internalRoots, externalRoots)
	}
	for _, id := range out.ExecutionPackages[0].Roots {
		p := nodes[id]
		if p.ImportPath == "packages.test/control/app" {
			found := false
			for _, dep := range p.Imports {
				found = found || nodes[dep].ImportPath == "packages.test/control/helper"
			}
			if !found {
				t.Fatal("lost initialization dependency")
			}
		}
	}
}

func TestPackageInventoryExampleFuzzAndBenchmarkOnly(t *testing.T) {
	root := testModule(t, map[string]string{
		"go.mod":            "module other.test/control\n\ngo 1.26.0\n",
		"example/e_test.go": "package example\nimport \"fmt\"\nfunc Example() { fmt.Println(1)\n// Output: 1\n}\n",
		"fuzz/f_test.go":    "package fuzz\nimport \"testing\"\nfunc FuzzValue(f *testing.F) { f.Add(1); f.Fuzz(func(t *testing.T, n int) {}) }\n",
		"bench/b_test.go":   "package bench\nimport \"testing\"\nfunc BenchmarkValue(b *testing.B) {}\n",
	})
	out := packageEvidence(t, root)
	if len(out.ExecutionPackages) != 3 {
		t.Fatalf("lost non-TestX binaries: %+v", out.ExecutionPackages)
	}
	ordinary := Discover(context.Background(), Options{SourceDir: root})
	if ordinary.Status != "ok" || len(ordinary.Tests) != 0 {
		t.Fatalf("ordinary inventory changed: %+v", ordinary)
	}
}

func TestPackageInventoryExternalOnlyRootsAreSourceBacked(t *testing.T) {
	for _, production := range []bool{false, true} {
		t.Run(fmt.Sprint(production), func(t *testing.T) {
			files := map[string]string{
				"go.mod":               "module externalonly.test/control\n\ngo 1.26.0\n",
				"app/external_test.go": "package app_test\nimport \"testing\"\nfunc TestOutside(t *testing.T) {}\n",
			}
			if production {
				files["app/app.go"] = "package app\nimport _ \"externalonly.test/control/helper\"\n"
				files["helper/helper.go"] = "package helper\nvar V = 1\n"
			}
			out := packageEvidence(t, testModule(t, files))
			if len(out.ExecutionPackages) != 1 {
				t.Fatalf("binary grouping: %+v", out.ExecutionPackages)
			}
			foundProduction := false
			for _, id := range out.ExecutionPackages[0].Roots {
				for _, node := range out.Packages {
					if node.ID != id {
						continue
					}
					if len(node.SourceFiles) == 0 {
						t.Fatalf("source-less root: %+v", node)
					}
					foundProduction = foundProduction || node.ImportPath == "externalonly.test/control/app"
				}
			}
			if foundProduction != production {
				t.Fatalf("production root present=%v want=%v", foundProduction, production)
			}
		})
	}
}

func TestPackageInventoryDoesNotLoadDependencySyntax(t *testing.T) {
	lc, err := prepareLoad(context.Background(), Options{SourceDir: packageFixture(t)})
	if err != nil {
		t.Fatal(err)
	}
	defer lc.Close()
	pkgs, err := loadPackageMetadata(lc)
	if err != nil {
		t.Fatal(err)
	}
	seen := map[string]bool{}
	var visit func(*packages.Package)
	visit = func(p *packages.Package) {
		if seen[p.ID] {
			return
		}
		seen[p.ID] = true
		if len(p.Syntax) != 0 || p.TypesInfo != nil {
			t.Fatalf("metadata loaded dependency syntax/types for %s", p.ID)
		}
		for _, dep := range p.Imports {
			visit(dep)
		}
	}
	for _, p := range pkgs {
		visit(p)
	}
	if len(seen) < 20 {
		t.Fatal("metadata did not load transitive dependencies")
	}
}

func TestPackageInventoryEffectiveDriverContext(t *testing.T) {
	t.Setenv("CGO_ENABLED", "0")
	out := packageEvidence(t, packageFixture(t))
	cmd := exec.Command("go", "env", "GOVERSION")
	cmd.Env = append(os.Environ(), "GOENV=off", "GOTOOLCHAIN=local")
	version, err := cmd.Output()
	if err != nil {
		t.Fatal(err)
	}
	c := out.EffectiveBuildContext
	if c == nil || c.GoVersion != strings.TrimSpace(string(version)) || c.CGOEnabled != "0" || c.Environment["GOWORK"] != "off" || c.Environment["GOENV"] != "off" {
		t.Fatalf("wrong effective context: %+v", c)
	}
	for _, f := range c.BuildFlags {
		if strings.Contains(f, "modfile=") {
			t.Fatal("ephemeral scratch path exposed as execution flag")
		}
	}
}

func TestPackageInventoryGeneratedHarnessProvenance(t *testing.T) {
	out := packageEvidence(t, packageFixture(t))
	harnesses := 0
	for _, p := range out.Packages {
		if p.Provenance.Kind == "generated_test_harness" {
			harnesses++
			if len(p.SourceFiles) != 0 {
				t.Fatal("generated harness reported as committed source")
			}
		}
		for _, f := range p.SourceFiles {
			rel, err := filepath.Rel(out.SourceRoot, f)
			if err != nil || rel == ".." || strings.HasPrefix(rel, "../") {
				t.Fatalf("foreign local source: %s", f)
			}
		}
	}
	if harnesses != 1 {
		t.Fatalf("want one generated test harness, got %d", harnesses)
	}
}
