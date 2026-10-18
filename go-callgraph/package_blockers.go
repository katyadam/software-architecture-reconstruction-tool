package main

import (
	"context"
	"errors"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"golang.org/x/tools/go/packages"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
)

// This scan is an additional local safeguard, not proof about opaque library
// side effects. In particular it does not certify arbitrary integration tests.
func scanPackageBlockers(ctx context.Context, root string, pkgs []*packages.Package) ([]PackageBlocker, error) {
	blockers := map[string]PackageBlocker{}
	add := func(code, path, why string) {
		blockers[code+"\x00"+path] = PackageBlocker{Code: code, SourcePath: path, Diagnostic: why}
	}
	seenFiles := map[string]bool{}
	failures := []error{}
	packages.Visit(pkgs, nil, func(p *packages.Package) {
		if err := ctx.Err(); err != nil {
			failures = append(failures, err)
			return
		}
		if p.Module == nil {
			return
		}
		if r := p.Module.Replace; r != nil && r.Version == "" && !localPath(root, r.Dir) {
			add("outside_local_replacement", "", "local replacement outside verified checkout: "+r.Dir)
			return
		}
		if !localPath(root, p.Dir) {
			return
		}
		for _, f := range p.OtherFiles {
			if localPath(root, f) {
				add("cgo_or_assembly", f, "active module-local non-Go input cannot be certified by package policy")
			}
		}
		for _, path := range p.GoFiles {
			if !localPath(root, path) || seenFiles[path] {
				continue
			}
			seenFiles[path] = true
			file, err := parser.ParseFile(token.NewFileSet(), path, nil, parser.ParseComments)
			if err != nil {
				add("uninspectable_local_sources", path, "cannot inspect active local Go source: "+err.Error())
				failures = append(failures, err)
				continue
			}
			aliases := map[string]string{}
			dot := map[string]bool{}
			for _, imp := range file.Imports {
				imported, err := strconv.Unquote(imp.Path.Value)
				if err != nil {
					failures = append(failures, err)
					add("uninspectable_local_sources", path, "invalid import path")
					continue
				}
				alias := filepath.Base(imported)
				if imp.Name != nil {
					alias = imp.Name.Name
				}
				if alias == "." {
					dot[imported] = true
				} else {
					aliases[alias] = imported
				}
				switch imported {
				case "plugin":
					add("dynamic_loading", path, "active local source imports plugin")
				case "os/exec":
					add("process_execution", path, "active local source imports os/exec")
				case "unsafe":
					add("unsafe_linkage", path, "active local source imports unsafe")
				case "C":
					add("cgo_or_assembly", path, "active local source imports C")
				}
			}
			for _, group := range file.Comments {
				for _, c := range group.List {
					if strings.HasPrefix(strings.TrimSpace(c.Text), "//go:linkname") {
						add("unsafe_linkage", path, "active local source contains go:linkname")
					}
				}
			}
			ast.Inspect(file, func(n ast.Node) bool {
				switch n := n.(type) {
				case *ast.FuncDecl:
					if strings.HasSuffix(path, "_test.go") && n.Recv == nil && n.Name.Name == "TestMain" {
						add("test_main", path, "TestMain may change cross-binary execution behavior")
					}
				case *ast.SelectorExpr:
					if qualifier, ok := n.X.(*ast.Ident); ok && processReference(aliases[qualifier.Name], n.Sel.Name) {
						add("process_execution", path, "active local source references a process-launch capability")
					}
				case *ast.Ident:
					for imported := range dot {
						if processReference(imported, n.Name) {
							add("process_execution", path, "active local source references a dot-imported process-launch capability")
						}
					}
				}
				return true
			})
		}
	})
	result := []PackageBlocker{}
	for _, b := range blockers {
		result = append(result, b)
	}
	sort.Slice(result, func(i, j int) bool { return result[i].Code+result[i].SourcePath < result[j].Code+result[j].SourcePath })
	return result, errors.Join(failures...)
}

func processReference(imported, member string) bool {
	if imported == "os" {
		return member == "StartProcess"
	}
	if imported != "syscall" {
		return false
	}
	switch member {
	case "Exec", "ForkExec", "StartProcess", "Syscall", "Syscall6", "Syscall9", "RawSyscall", "RawSyscall6", "RawSyscallNoError", "SyscallNoError":
		return true
	default:
		return false
	}
}

func failedPackageScan(out *PackageInventoryOutcome, err error) {
	out.Blockers = append(out.Blockers, PackageBlocker{Code: "uninspectable_local_sources", Diagnostic: fmt.Sprintf("package inventory could not establish complete active-source inspection: %v", err)})
	out.Diagnostics = append(out.Diagnostics, err.Error())
}
