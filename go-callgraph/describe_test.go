package main

import (
	"context"
	"testing"
)

func TestDescribeOneLineMethodAndClosure(t *testing.T) {
	out := DescribeSources(context.Background(), []SourceInput{{SourcePath: "baseline:s.go", Source: "package p\ntype T struct{}\nfunc (v *T) Value() int { f:=func() int{return 1}; return f() }"}, {SourcePath: "candidate:s.go", Source: "package p\ntype T struct{}\nfunc (v *T) Value() int { f:=func() int{return 2}; return f() }"}})
	if out.Status != "ok" || len(out.Files) != 2 {
		t.Fatalf("describe: %+v", out)
	}
	a, b := out.Files[0].Declarations, out.Files[1].Declarations
	if len(a) != 1 || len(b) != 1 || a[0].MemberName != "(*T).Value" || a[0].Header != b[0].Header || a[0].StartLine != 3 || a[0].BodyStartLine != 3 || a[0].BodyEndLine != 3 {
		t.Fatalf("spans/header: %+v %+v", a, b)
	}
}
func TestDescribeBadSourceIsExplicit(t *testing.T) {
	out := DescribeSources(context.Background(), []SourceInput{{SourcePath: "bad.go", Source: "package p; func broken("}})
	if len(out.Files) != 1 || len(out.Files[0].Diagnostics) == 0 {
		t.Fatalf("parse error lost: %+v", out)
	}
}
