package main

import (
	"context"
	"fmt"
	"go/ast"
	"go/types"
	"os"
	"runtime"
	"sort"
	"strings"
	"time"
	"unicode"
	"unicode/utf8"

	"golang.org/x/tools/go/callgraph"
	"golang.org/x/tools/go/callgraph/cha"
	"golang.org/x/tools/go/ssa"
	"golang.org/x/tools/go/ssa/ssautil"
)

func emptyOutcome(opts Options) Outcome {
	return Outcome{SchemaVersion: 2, Status: "failed", ProviderID: "go-native", SourceRoot: opts.SourceDir,
		Algorithm: "cha", Diagnostics: []string{}, Reachability: []Reachability{}, Edges: []any{}, Tests: []MethodRef{},
		Scope: Scope{DependencyBodies: "opaque", GOOS: envOr("GOOS", runtime.GOOS), GOARCH: envOr("GOARCH", runtime.GOARCH),
			GoVersion: runtime.Version(), BuildTags: opts.Tags, WorkspaceMode: "off",
			Limitations: []string{"external-library callbacks", "reflection and plugins", "generic dispatch", "TestMain unsupported", "proof only; not safe for production test exclusion"}}}
}

func envOr(key, fallback string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return fallback
}

func Analyze(ctx context.Context, opts Options) (out Outcome) {
	out = emptyOutcome(opts)
	defer func() {
		if problem := recover(); problem != nil {
			out.Status = "failed"
			out.Diagnostics = append(out.Diagnostics, fmt.Sprintf("analysis panic: %v", problem))
			out.Reachability = []Reachability{}
		}
	}()
	failed := func(err error) Outcome {
		out.Status = "failed"
		out.Diagnostics = append(out.Diagnostics, err.Error())
		out.Reachability = []Reachability{}
		return out
	}
	start := time.Now()
	phase := func(name string) {
		if opts.Progress != nil {
			fmt.Fprintf(opts.Progress, "callgraph_phase=%s provider=go-native elapsed_ms=%d\n", name, time.Since(start).Milliseconds())
		}
	}
	phase("loading")
	root, pkgs, err := loadSource(ctx, opts)
	out.SourceRoot = root
	if err != nil {
		return failed(err)
	}
	if err = ctx.Err(); err != nil {
		return failed(err)
	}
	phase("building")
	prog, _ := ssautil.Packages(pkgs, ssa.InstantiateGenerics)
	prog.Build()
	graph := cha.CallGraph(prog)
	out.Diagnostics = append(out.Diagnostics, addSortCallbackEdges(graph)...)
	phase("graph_ready")
	refFor := func(fn *ssa.Function) (MethodRef, bool) {
		if fn == nil {
			return MethodRef{}, false
		}
		if origin := fn.Origin(); origin != nil {
			fn = origin
		}
		obj, ok := fn.Object().(*types.Func)
		if !ok || obj.Pkg() == nil {
			return MethodRef{}, false
		}
		return sourceRef(prog.Fset, obj, root)
	}
	// A package can appear in production and test-augmented forms. Group by
	// source declaration while preserving every SSA variant for traversal.
	targetNodes := map[string][]*callgraph.Node{}
	targetRefs := map[string]MethodRef{}
	for fn, node := range graph.Nodes {
		if ref, ok := refFor(fn); ok {
			key := ref.DeclaringType + "#" + ref.MemberName
			targetNodes[key] = append(targetNodes[key], node)
			targetRefs[key] = ref
		}
	}
	roots := map[*callgraph.Node]map[MethodRef]bool{}
	tests := map[MethodRef]bool{}
	attach := func(fn *ssa.Function, ref MethodRef) {
		if node := graph.Nodes[fn]; node != nil {
			if roots[node] == nil {
				roots[node] = map[MethodRef]bool{}
			}
			roots[node][ref] = true
		}
	}
	for _, pkg := range pkgs {
		for _, file := range pkg.Syntax {
			if !strings.HasSuffix(pkg.Fset.PositionFor(file.Pos(), false).Filename, "_test.go") {
				continue
			}
			for _, decl := range file.Decls {
				fd, ok := decl.(*ast.FuncDecl)
				if !ok || fd.Recv != nil {
					continue
				}
				obj, ok := pkg.TypesInfo.Defs[fd.Name].(*types.Func)
				if !ok {
					continue
				}
				fn := prog.FuncValue(obj)
				if fn == nil {
					continue
				}
				ref, local := refFor(fn)
				if !local {
					continue
				}
				if fd.Name.Name == "TestMain" {
					return failed(fmt.Errorf("TestMain setup is unsupported: %s", ref.SourcePath))
				}
				if !isTest(obj) {
					continue
				}
				tests[ref] = true
				attach(fn, ref)
				seen := map[*types.Package]bool{}
				var initRoots func(*types.Package)
				initRoots = func(p *types.Package) {
					if seen[p] {
						return
					}
					seen[p] = true
					if sp := prog.Package(p); sp != nil {
						if initFn, ok := sp.Members["init"].(*ssa.Function); ok {
							attach(initFn, ref)
						}
					}
					for _, imported := range p.Imports() {
						initRoots(imported)
					}
				}
				initRoots(pkg.Types)
				// Every package linked into the same test binary initializes
				// before any internal or external test in that binary runs.
				if pkg.ForTest != "" {
					for _, sibling := range pkgs {
						if sibling.ForTest == pkg.ForTest {
							initRoots(sibling.Types)
						}
					}
				}
			}
		}
	}
	out.Tests = sortedRefs(tests)
	for _, key := range opts.Targets {
		// The traversal stack is mutable; do not overwrite cached seed nodes
		// when a caller requests the same target more than once.
		nodes := append([]*callgraph.Node(nil), targetNodes[key]...)
		if len(nodes) == 0 {
			return failed(fmt.Errorf("unmatched target: %s", key))
		}
		visited := map[*callgraph.Node]bool{}
		found := map[MethodRef]bool{}
		for len(nodes) > 0 {
			if err = ctx.Err(); err != nil {
				return failed(err)
			}
			n := nodes[len(nodes)-1]
			nodes = nodes[:len(nodes)-1]
			if visited[n] {
				continue
			}
			visited[n] = true
			for ref := range roots[n] {
				found[ref] = true
			}
			for _, edge := range n.In {
				nodes = append(nodes, edge.Caller)
			}
		}
		out.Reachability = append(out.Reachability, Reachability{Method: targetRefs[key], Entrypoints: sortedRefs(found)})
	}
	sort.Slice(out.Reachability, func(i, j int) bool {
		a, b := out.Reachability[i].Method, out.Reachability[j].Method
		return a.DeclaringType+"#"+a.MemberName < b.DeclaringType+"#"+b.MemberName
	})
	out.Status = "ok"
	if len(tests) == 0 {
		out.Status = "no_entrypoints"
	}
	phase("reachability_ready")
	return out
}

func isTest(obj *types.Func) bool {
	name := obj.Name()
	if !strings.HasPrefix(name, "Test") {
		return false
	}
	if suffix := strings.TrimPrefix(name, "Test"); suffix != "" {
		r, _ := utf8.DecodeRuneInString(suffix)
		if unicode.IsLower(r) {
			return false
		}
	}
	sig := obj.Type().(*types.Signature)
	if sig.Params().Len() != 1 || sig.Results().Len() != 0 || sig.TypeParams().Len() != 0 {
		return false
	}
	ptr, ok := types.Unalias(sig.Params().At(0).Type()).(*types.Pointer)
	if !ok {
		return false
	}
	named, ok := types.Unalias(ptr.Elem()).(*types.Named)
	return ok && named.Obj().Pkg() != nil && named.Obj().Pkg().Path() == "testing" && named.Obj().Name() == "T"
}

func sortedRefs(set map[MethodRef]bool) []MethodRef {
	refs := []MethodRef{}
	for ref := range set {
		refs = append(refs, ref)
	}
	sort.Slice(refs, func(i, j int) bool {
		a, b := refs[i], refs[j]
		return a.DeclaringType+"#"+a.MemberName+"#"+a.SourcePath < b.DeclaringType+"#"+b.MemberName+"#"+b.SourcePath
	})
	return refs
}
