package main

import (
	"context"
	"fmt"
	"golang.org/x/mod/modfile"
	"golang.org/x/tools/go/packages"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

// A scratch modfile protects even go.sum: -mod=readonly alone may update sums.
func loadSource(ctx context.Context, opts Options) (string, []*packages.Package, error) {
	root, err := filepath.Abs(opts.SourceDir)
	if err != nil {
		return root, nil, err
	}
	root, err = filepath.EvalSymlinks(root)
	if err != nil {
		return root, nil, err
	}
	data, err := os.ReadFile(filepath.Join(root, "go.mod"))
	if err != nil {
		return root, nil, fmt.Errorf("source-dir must be a module root: %w", err)
	}
	mf, err := modfile.Parse("go.mod", data, nil)
	if err != nil {
		return root, nil, err
	}
	for _, r := range mf.Replace {
		if r.New.Version == "" && !filepath.IsAbs(r.New.Path) {
			if err := mf.AddReplace(r.Old.Path, r.Old.Version, filepath.Join(root, r.New.Path), ""); err != nil {
				return root, nil, err
			}
		}
	}
	data, err = mf.Format()
	if err != nil {
		return root, nil, err
	}
	scratch, err := os.MkdirTemp("", "go-callgraph-module-")
	if err != nil {
		return root, nil, err
	}
	defer os.RemoveAll(scratch)
	modpath := filepath.Join(scratch, "analysis.mod")
	if err = os.WriteFile(modpath, data, 0600); err != nil {
		return root, nil, err
	}
	if sum, e := os.ReadFile(filepath.Join(root, "go.sum")); e == nil {
		if err = os.WriteFile(filepath.Join(scratch, "analysis.sum"), sum, 0600); err != nil {
			return root, nil, err
		}
	} else if !os.IsNotExist(e) {
		return root, nil, e
	}
	env := []string{}
	for _, v := range os.Environ() {
		k, _, _ := strings.Cut(v, "=")
		switch k {
		case "GOWORK", "GOTOOLCHAIN", "GOPROXY", "GOSUMDB", "GOENV", "GOFLAGS", "GOOS", "GOARCH":
		default:
			env = append(env, v)
		}
	}
	// Saved `go env -w` flags/platforms must not silently override reported
	// context. Only explicit tags and process GOOS/GOARCH select this build.
	scope := emptyOutcome(opts).Scope
	env = append(env, "GOWORK=off", "GOTOOLCHAIN=local", "GOPROXY=off", "GOSUMDB=off",
		"GOENV=off", "GOFLAGS=", "GOOS="+scope.GOOS, "GOARCH="+scope.GOARCH)
	flags := []string{"-mod=readonly", "-modfile=" + modpath}
	if opts.Tags != "" {
		flags = append(flags, "-tags="+opts.Tags)
	}
	pkgs, err := packages.Load(&packages.Config{Context: ctx, Dir: root, Env: env, Mode: packages.LoadSyntax | packages.NeedForTest, Tests: true, BuildFlags: flags}, "./...")
	if err != nil {
		return root, nil, err
	}
	problems := map[string]bool{}
	packages.Visit(pkgs, nil, func(p *packages.Package) {
		for _, e := range p.Errors {
			problems[e.Error()] = true
		}
	})
	messages := []string{}
	for e := range problems {
		messages = append(messages, e)
	}
	sort.Strings(messages)
	if len(messages) > 0 {
		return root, nil, fmt.Errorf("Go package loading: %s", strings.Join(messages, "; "))
	}
	if err = ctx.Err(); err != nil {
		return root, nil, err
	}
	return root, pkgs, nil
}
