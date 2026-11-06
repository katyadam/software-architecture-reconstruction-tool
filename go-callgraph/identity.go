package main

import (
	"go/token"
	"go/types"
	"path/filepath"
	"strings"
)

func localPath(root, path string) bool {
	rel, err := filepath.Rel(root, path)
	return err == nil && rel != ".." && !strings.HasPrefix(rel, ".."+string(filepath.Separator))
}
func sourceRef(fset *token.FileSet, obj *types.Func, root string) (MethodRef, bool) {
	if obj == nil || obj.Pkg() == nil {
		return MethodRef{}, false
	}
	// Git diffs and ownership refer to physical files, not //line aliases.
	pos := fset.PositionFor(obj.Pos(), false)
	if !pos.IsValid() || !localPath(root, pos.Filename) {
		return MethodRef{}, false
	}
	member := obj.Name()
	sig := obj.Type().(*types.Signature)
	if r := sig.Recv(); r != nil {
		member = "(" + types.TypeString(r.Type(), func(*types.Package) string { return "" }) + ")." + member
	}
	return MethodRef{Language: "go", DeclaringType: obj.Pkg().Path(), MemberName: member, Descriptor: types.TypeString(obj.Type(), func(p *types.Package) string { return p.Path() }), SourcePath: pos.Filename, SourceLine: pos.Line}, true
}
