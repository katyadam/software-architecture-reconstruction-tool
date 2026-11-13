package main

import (
	"context"
	"reflect"
	"testing"
)

// An opaque CancelFunc has signature func(), so module-wide CHA invents a
// call to an unrelated executable's main. Binary-scoped traversal must not
// accept that path, even if the final target itself is linked into the test.
func TestCHAPathsCannotCrossAnUnlinkedExecutable(t *testing.T) {
	for _, linked := range []bool{false, true} {
		name, imported := "unlinked-target", ""
		if linked {
			name, imported = "linked-target-unlinked-bridge", `import _ "binaryscope.test/control/changed"`
		}
		t.Run(name, func(t *testing.T) {
			root := testModule(t, map[string]string{
				"go.mod":             "module binaryscope.test/control\n\ngo 1.26.0\n",
				"changed/value.go":   "package changed\nfunc Value() int { return 1 }\n",
				"cmd/server/main.go": "package main\nimport \"binaryscope.test/control/changed\"\nfunc main() { _ = changed.Value() }\n",
				"client/client.go":   "package client\n" + imported + "\n",
				"client/client_test.go": `package client
import ("context"; "testing")
func TestCancel(t *testing.T) { _, cancel := context.WithCancel(context.Background()); defer cancel() }
`,
			})
			out := Analyze(context.Background(), Options{SourceDir: root, Targets: []string{"binaryscope.test/control/changed#Value"}})
			if out.Status != "ok" || len(out.Tests) != 1 || len(out.Reachability) != 1 {
				t.Fatalf("analysis: %+v", out)
			}
			if got := names(out.Reachability[0].Entrypoints); len(got) != 0 {
				t.Fatalf("unlinked executable created a false path: %v", got)
			}
		})
	}
}

func TestBinaryScopeRetainsImportedFunctionValuesAndBoundMethods(t *testing.T) {
	root := testModule(t, map[string]string{
		"go.mod": "module binaryscope.test/control\n\ngo 1.26.0\n",
		"changed/value.go": `package changed
func Value() int { return 1 }
type Worker struct{}
func (Worker) Method() int { return Value() }
`,
		"client/client_test.go": `package client
import ("testing"; "binaryscope.test/control/changed")
func invoke(fn func() int) int { return fn() }
func TestFunction(t *testing.T) { if invoke(changed.Value) != 1 { t.Fatal("value") } }
func TestBoundMethod(t *testing.T) { if invoke(changed.Worker{}.Method) != 1 { t.Fatal("method") } }
`,
	})
	out := Analyze(context.Background(), Options{SourceDir: root, Targets: []string{"binaryscope.test/control/changed#Value"}})
	if out.Status != "ok" || len(out.Reachability) != 1 {
		t.Fatalf("analysis: %+v", out)
	}
	if got := names(out.Reachability[0].Entrypoints); !reflect.DeepEqual(got, []string{"TestBoundMethod", "TestFunction"}) {
		t.Fatalf("linked callbacks were lost: %v", got)
	}
}

func TestBinaryScopeIncludesProductionInitializationForExternalOnlyTests(t *testing.T) {
	root := testModule(t, map[string]string{
		"go.mod":               "module binaryscope.test/control\n\ngo 1.26.0\n",
		"changed/value.go":     "package changed\nfunc Value() int { return 1 }\n",
		"app/app.go":           "package app\nimport \"binaryscope.test/control/changed\"\nvar seed = changed.Value()\n",
		"app/external_test.go": "package app_test\nimport \"testing\"\nfunc TestExternal(t *testing.T) {}\n",
	})
	out := Analyze(context.Background(), Options{SourceDir: root, Targets: []string{"binaryscope.test/control/changed#Value"}})
	if out.Status != "ok" || len(out.Reachability) != 1 {
		t.Fatalf("analysis: %+v", out)
	}
	if got := names(out.Reachability[0].Entrypoints); !reflect.DeepEqual(got, []string{"TestExternal"}) {
		t.Fatalf("production initialization was lost: %v", got)
	}
}
