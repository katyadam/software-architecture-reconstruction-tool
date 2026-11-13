package main

import (
	"go/types"

	"golang.org/x/tools/go/callgraph"
	"golang.org/x/tools/go/packages"
	"golang.org/x/tools/go/ssa"
)

// CHA builds one module-wide graph. Its signature-only function-value edges
// may enter executables or sibling tests that are not linked into this binary.
// Keep traversal contexts separate; do not globally delete shared graph edges.
type binaryScope struct {
	packages map[*types.Package]bool
	roots    map[*callgraph.Node]map[MethodRef]bool
}

func newBinaryScope(execution string, testPackage *packages.Package, loaded []*packages.Package) *binaryScope {
	scope := &binaryScope{packages: map[*types.Package]bool{}, roots: map[*callgraph.Node]map[MethodRef]bool{}}
	var visit func(*types.Package)
	visit = func(p *types.Package) {
		if p == nil || scope.packages[p] {
			return
		}
		scope.packages[p] = true
		for _, imported := range p.Imports() {
			visit(imported)
		}
	}
	visit(testPackage.Types)
	augmented := false
	for _, p := range loaded {
		if p.ForTest == execution {
			visit(p.Types)
			augmented = augmented || p.PkgPath == execution
		}
	}
	// The generated harness imports production even if external-only tests do
	// not import it. Prefer the augmented instance when internal tests exist.
	if !augmented {
		for _, p := range loaded {
			if p.PkgPath == execution && p.ForTest == "" && len(p.GoFiles) > 0 {
				visit(p.Types)
			}
		}
	}
	return scope
}

func (scope *binaryScope) contains(fn *ssa.Function) bool {
	for fn != nil {
		if fn.Pkg != nil {
			return scope.packages[fn.Pkg.Pkg]
		}
		if obj := fn.Object(); obj != nil && obj.Pkg() != nil {
			return scope.packages[obj.Pkg()]
		}
		fn = fn.Parent()
	}
	// A synthetic wrapper without a recoverable owner is uncertain. Retain it;
	// any later edge into a known package still passes through this boundary.
	return true
}
