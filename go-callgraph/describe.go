package main

import (
	"bytes"
	"context"
	"fmt"
	"go/ast"
	"go/format"
	"go/parser"
	"go/token"
)

func declaration(fset *token.FileSet, fd *ast.FuncDecl) SourceDeclaration {
	var b bytes.Buffer
	// Serialize only semantic header nodes, not function bodies or doc comments.
	name := fd.Name.Name
	if fd.Recv != nil && len(fd.Recv.List) == 1 {
		var recv bytes.Buffer
		_ = format.Node(&recv, fset, fd.Recv.List[0].Type)
		name = "(" + recv.String() + ")." + name
	}
	_ = format.Node(&b, fset, fd.Type)
	out := SourceDeclaration{MemberName: name, Header: name + ":" + b.String(), StartLine: fset.Position(fd.Pos()).Line, EndLine: fset.Position(fd.End()).Line}
	if fd.Body != nil {
		out.BodyStartLine = fset.Position(fd.Body.Pos()).Line
		out.BodyEndLine = fset.Position(fd.Body.End()).Line
	}
	return out
}
func DescribeSources(ctx context.Context, files []SourceInput) DescriptionOutcome {
	out := DescriptionOutcome{SchemaVersion: 1, ProviderID: "go-native", Status: "ok", Files: []FileDescription{}, Diagnostics: []string{}}
	for _, input := range files {
		if err := ctx.Err(); err != nil {
			out.Status = "failed"
			out.Diagnostics = append(out.Diagnostics, err.Error())
			break
		}
		item := FileDescription{SourcePath: input.SourcePath, Declarations: []SourceDeclaration{}, Diagnostics: []string{}}
		fset := token.NewFileSet()
		file, err := parser.ParseFile(fset, input.SourcePath, input.Source, parser.AllErrors)
		if err != nil {
			item.Diagnostics = append(item.Diagnostics, fmt.Sprint(err))
		} else {
			name := file.Name.Name
			item.PackageName = &name
			for _, d := range file.Decls {
				if fd, ok := d.(*ast.FuncDecl); ok {
					item.Declarations = append(item.Declarations, declaration(fset, fd))
				}
			}
		}
		out.Files = append(out.Files, item)
	}
	return out
}
