package main

import (
	"context"
	"fmt"
	"go/ast"
	"go/types"
	"sort"
	"strings"
	"time"
)

func Discover(ctx context.Context, opts Options) InventoryOutcome {
	out := InventoryOutcome{SchemaVersion: 1, ProviderID: "go-native", Status: "failed", SourceRoot: opts.SourceDir, SourceFiles: []string{}, Tests: []NativeTest{}, Declarations: []NativeDeclaration{}, Scope: emptyOutcome(opts).Scope, Diagnostics: []string{}}
	start := time.Now()
	phase := func(name string) {
		if opts.Progress != nil {
			fmt.Fprintf(opts.Progress, "callgraph_phase=inventory_%s provider=go-native elapsed_ms=%d\n", name, time.Since(start).Milliseconds())
		}
	}
	phase("loading")
	root, pkgs, err := loadSource(ctx, opts)
	out.SourceRoot = root
	if err != nil {
		out.Diagnostics = append(out.Diagnostics, err.Error())
		return out
	}
	files := map[string]bool{}
	tests := map[MethodRef]NativeTest{}
	decls := map[MethodRef]NativeDeclaration{}
	for _, pkg := range pkgs {
		for _, path := range pkg.GoFiles {
			if localPath(root, path) {
				files[path] = true
			}
		}
		for _, file := range pkg.Syntax {
			path := pkg.Fset.Position(file.Pos()).Filename
			if !localPath(root, path) {
				continue
			}
			for _, d := range file.Decls {
				fd, ok := d.(*ast.FuncDecl)
				if !ok {
					continue
				}
				obj, ok := pkg.TypesInfo.Defs[fd.Name].(*types.Func)
				if !ok {
					continue
				}
				ref, ok := sourceRef(pkg.Fset, obj, root)
				if !ok {
					continue
				}
				decls[ref] = NativeDeclaration{Method: ref, Syntax: declaration(pkg.Fset, fd)}
				if strings.HasSuffix(path, "_test.go") && fd.Recv == nil {
					if obj.Name() == "TestMain" {
						out.Diagnostics = append(out.Diagnostics, "TestMain setup remains unsupported for graph exclusion")
					}
					if isTest(obj) {
						execution := pkg.ForTest
						if execution == "" {
							execution = pkg.PkgPath
						}
						tests[ref] = NativeTest{Method: ref, ExecutionPackage: execution}
					}
				}
			}
		}
	}
	for f := range files {
		out.SourceFiles = append(out.SourceFiles, f)
	}
	sort.Strings(out.SourceFiles)
	for _, v := range tests {
		out.Tests = append(out.Tests, v)
	}
	sort.Slice(out.Tests, func(i, j int) bool {
		a, b := out.Tests[i].Method, out.Tests[j].Method
		return a.DeclaringType+"#"+a.MemberName < b.DeclaringType+"#"+b.MemberName
	})
	for _, v := range decls {
		out.Declarations = append(out.Declarations, v)
	}
	sort.Slice(out.Declarations, func(i, j int) bool {
		a, b := out.Declarations[i].Method, out.Declarations[j].Method
		return a.SourcePath+fmt.Sprint(a.SourceLine)+a.MemberName < b.SourcePath+fmt.Sprint(b.SourceLine)+b.MemberName
	})
	out.Status = "ok"
	out.TestInventoryComplete = true
	phase("ready")
	return out
}
