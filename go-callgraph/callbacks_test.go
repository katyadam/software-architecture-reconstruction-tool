package main

import (
	"context"
	"fmt"
	"reflect"
	"strings"
	"testing"
)

// Missing callback edges lose TestChanged; edges via the shared sort function
// incorrectly join the two independent sorting callers.
func TestSortCallbacksPreserveCallerSpecificReachability(t *testing.T) {
	for _, operation := range []string{"Slice", "SliceStable"} {
		for _, callback := range []struct{ name, expression string }{
			{"named", "Changed"},
			{"closure", "func(i, j int) bool { return Changed(values[i], values[j]) }"},
		} {
			t.Run(operation+"/"+callback.name, func(t *testing.T) {
				root := testModule(t, map[string]string{
					"go.mod": "module callbacks.test/control\n\ngo 1.26.0\n",
					"sort.go": fmt.Sprintf(`package control
import "sort"
func Changed(a, b int) bool { return a < b }
func Other(a, b int) bool { return a > b }
func Uncalled(a, b int) bool { return a == b }
func SortChanged(values []int) { sort.%s(values, %s) }
func SortOther(values []int) { sort.%s(values, func(i, j int) bool { return Other(values[i], values[j]) }) }
`, operation, callback.expression, operation),
					"sort_test.go": `package control
import "testing"
func TestChanged(t *testing.T) { SortChanged([]int{2, 1}) }
func TestOther(t *testing.T) { SortOther([]int{2, 1}) }
func TestUnrelated(t *testing.T) {}
`,
				})
				out := Analyze(context.Background(), Options{SourceDir: root, Targets: []string{
					"callbacks.test/control#Changed", "callbacks.test/control#Other", "callbacks.test/control#Uncalled",
				}})
				if out.Status != "ok" || len(out.Tests) != 3 || len(out.Reachability) != 3 {
					t.Fatalf("analysis: %+v", out)
				}
				want := map[string][]string{"Changed": {"TestChanged"}, "Other": {"TestOther"}, "Uncalled": {}}
				for _, record := range out.Reachability {
					if got := names(record.Entrypoints); !reflect.DeepEqual(got, want[record.Method.MemberName]) {
						t.Errorf("%s entrypoints=%v, want %v", record.Method.MemberName, got, want[record.Method.MemberName])
					}
				}
			})
		}
	}
}

func TestUnresolvedSortCallbackIsExplicit(t *testing.T) {
	root := testModule(t, map[string]string{
		"go.mod": "module callbacks.test/control\n\ngo 1.26.0\n",
		"sort.go": `package control
import "sort"
func Changed(a, b int) bool { return a < b }
func SortUnknown(values []int, less func(int,int)bool) { sort.Slice(values, less) }
`,
		"sort_test.go": `package control
import "testing"
func TestUnknown(t *testing.T) { SortUnknown([]int{2, 1}, Changed) }
`,
	})
	out := Analyze(context.Background(), Options{SourceDir: root, Targets: []string{"callbacks.test/control#Changed"}})
	if out.Status != "ok" || len(out.Tests) != 1 {
		t.Fatalf("analysis: %+v", out)
	}
	for _, diagnostic := range out.Diagnostics {
		if strings.Contains(diagnostic, "unresolved sort.Slice callback") {
			return
		}
	}
	t.Fatalf("unresolved callback was silent: %+v", out)
}
