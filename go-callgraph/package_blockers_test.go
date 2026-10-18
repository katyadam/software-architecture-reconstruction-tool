package main

import (
	"context"
	"golang.org/x/tools/go/packages"
	"os"
	"path/filepath"
	"slices"
	"testing"
)

func TestPackageBlockers(t *testing.T) {
	for _, tc := range []struct{ name, code, want string }{
		{"test_main", "package c; import \"testing\"; func TestMain(m *testing.M) { m.Run() }", "test_main"},
		{"plugin_alias", "package c; import p \"plugin\"; var Open = p.Open", "dynamic_loading"},
		{"exec_alias", "package c; import e \"os/exec\"; var Command = e.Command", "process_execution"},
		{"unsafe", "package c; import _ \"unsafe\"", "unsafe_linkage"},
		{"linkname", "package c\n//go:linkname aliased external\nfunc aliased()", "unsafe_linkage"},
		{"os_value", "package c; import o \"os\"; var Launch = o.StartProcess", "process_execution"},
		{"dot_value", "package c; import . \"os\"; var Launch = StartProcess", "process_execution"},
		{"syscall_value", "package c; import s \"syscall\"; var Launch = s.ForkExec", "process_execution"},
		{"raw_syscall", "package c; import s \"syscall\"; var Launch = s.RawSyscall", "process_execution"},
		{"windows_create_process", "package c; import s \"syscall\"; var Launch = s.CreateProcess", "process_execution"},
		{"windows_create_process_as_user", "package c; import . \"syscall\"; var Launch = CreateProcessAsUser", "process_execution"},
		{"windows_syscall12", "package c; import s \"syscall\"; var Launch = s.Syscall12", "process_execution"},
		{"windows_syscall15", "package c; import s \"syscall\"; var Launch = s.Syscall15", "process_execution"},
		{"windows_syscall18", "package c; import s \"syscall\"; var Launch = s.Syscall18", "process_execution"},
		{"windows_syscall_n", "package c; import s \"syscall\"; var Launch = s.SyscallN", "process_execution"},
		{"cgo", "package c\nimport \"C\"", "cgo_or_assembly"},
		{"invalid", "package c\nfunc broken( {", "uninspectable_local_sources"},
		{"safe_signal", "package c; import \"syscall\"; var Sig = syscall.SIGTERM", ""},
		{"safe_callbacks", "package c; import (\"sort\"; \"reflect\"); func Work() { var x []int; sort.Slice(x, func(i,j int) bool {return x[i]<x[j]}); _ = reflect.TypeOf(x) }", ""},
	} {
		t.Run(tc.name, func(t *testing.T) {
			root := testModule(t, map[string]string{"go.mod": "module blockers.test/c\n\ngo 1.26.0\n", "s_test.go": tc.code})
			pkgs := []*packages.Package{{ID: "blockers.test/c", Dir: root, GoFiles: []string{filepath.Join(root, "s_test.go")}, Module: &packages.Module{Main: true, Path: "blockers.test/c", Dir: root}}}
			blockers, err := scanPackageBlockers(context.Background(), root, pkgs)
			codes := []string{}
			for _, b := range blockers {
				codes = append(codes, b.Code)
			}
			if tc.want == "" && (err != nil || len(blockers) != 0) {
				t.Fatalf("false blocker: %v %v", blockers, err)
			}
			if tc.want != "" && !slices.Contains(codes, tc.want) {
				t.Fatalf("missing %s: %v %v", tc.want, blockers, err)
			}
		})
	}
	t.Run("assembly", func(t *testing.T) {
		root := testModule(t, map[string]string{"go.mod": "module asm.test/c\n\ngo 1.26.0\n", "s.go": "package c", "s.s": "// local assembler\n"})
		out := DiscoverPackages(context.Background(), Options{SourceDir: root})
		if !slices.ContainsFunc(out.Blockers, func(b PackageBlocker) bool { return b.Code == "cgo_or_assembly" }) {
			t.Fatalf("assembly missed: %v %v", out.Blockers, out.Diagnostics)
		}
	})
	t.Run("outside_replacement", func(t *testing.T) {
		dep := testModule(t, map[string]string{"go.mod": "module external.test/dep\n\ngo 1.26.0\n", "d.go": "package dep"})
		root := testModule(t, map[string]string{"go.mod": "module outside.test/c\n\ngo 1.26.0\nrequire external.test/dep v0.0.0\nreplace external.test/dep => " + dep + "\n", "s.go": "package c; import _ \"external.test/dep\""})
		out := DiscoverPackages(context.Background(), Options{SourceDir: root})
		if !slices.ContainsFunc(out.Blockers, func(b PackageBlocker) bool { return b.Code == "outside_local_replacement" }) {
			t.Fatalf("outside replacement hidden: %v %v", out.Blockers, out.Diagnostics)
		}
	})
	t.Run("unreadable", func(t *testing.T) {
		root := t.TempDir()
		pkgs := []*packages.Package{{Dir: root, GoFiles: []string{filepath.Join(root, "missing.go")}, Module: &packages.Module{Main: true}}}
		blockers, err := scanPackageBlockers(context.Background(), root, pkgs)
		if err == nil || !slices.ContainsFunc(blockers, func(b PackageBlocker) bool { return b.Code == "uninspectable_local_sources" }) {
			t.Fatalf("lost scan error: %v %v", blockers, err)
		}
	})
	t.Run("invalid_capability", func(t *testing.T) {
		root := testModule(t, map[string]string{"go.mod": "module invalid.test/c\n\ngo 1.26.0\n", "s.go": "package c\nfunc broken( {"})
		out := DiscoverPackages(context.Background(), Options{SourceDir: root})
		if out.Complete || out.Status != "failed" {
			t.Fatal("invalid source granted complete capability")
		}
	})
	t.Run("ignored_escape", func(t *testing.T) {
		root := testModule(t, map[string]string{"go.mod": "module inactive.test/c\n\ngo 1.26.0\n", "s.go": "package c", "inactive.go": "//go:build excluded\n\npackage c; import _ \"unsafe\""})
		out := DiscoverPackages(context.Background(), Options{SourceDir: root})
		if out.Status != "ok" || !out.Complete || len(out.Blockers) != 0 {
			t.Fatalf("scanned inactive source: %v %v", out.Blockers, out.Diagnostics)
		}
		if _, err := os.Stat(filepath.Join(root, "go.sum")); !os.IsNotExist(err) {
			t.Fatal("metadata created go.sum")
		}
	})
}
