package main

import (
	"context"
	"encoding/json"
	"fmt"
	"golang.org/x/mod/modfile"
	"golang.org/x/tools/go/packages"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
)

type loadContext struct {
	Root      string
	Config    packages.Config
	Effective EffectiveBuildContext
	scratch   string
	goEnv     map[string]string
}

func (l *loadContext) Close() { os.RemoveAll(l.scratch) }

// Scratch module/sum files prevent loader writes to the user's module.
func prepareLoad(ctx context.Context, opts Options) (_ *loadContext, err error) {
	root, err := filepath.Abs(opts.SourceDir)
	if err != nil {
		return nil, err
	}
	root, err = filepath.EvalSymlinks(root)
	if err != nil {
		return nil, err
	}
	data, err := os.ReadFile(filepath.Join(root, "go.mod"))
	if err != nil {
		return nil, fmt.Errorf("source-dir must be a module root: %w", err)
	}
	mf, err := modfile.Parse("go.mod", data, nil)
	if err != nil {
		return nil, err
	}
	for _, r := range mf.Replace {
		if r.New.Version == "" && !filepath.IsAbs(r.New.Path) {
			if err := mf.AddReplace(r.Old.Path, r.Old.Version, filepath.Join(root, r.New.Path), ""); err != nil {
				return nil, err
			}
		}
	}
	data, err = mf.Format()
	if err != nil {
		return nil, err
	}
	scratch, err := os.MkdirTemp("", "go-callgraph-module-")
	if err != nil {
		return nil, err
	}
	l := &loadContext{Root: root, scratch: scratch}
	defer func() {
		if err != nil {
			l.Close()
		}
	}()
	modpath := filepath.Join(scratch, "analysis.mod")
	if err = os.WriteFile(modpath, data, 0600); err != nil {
		return nil, err
	}
	if sum, e := os.ReadFile(filepath.Join(root, "go.sum")); e == nil {
		if err = os.WriteFile(filepath.Join(scratch, "analysis.sum"), sum, 0600); err != nil {
			return nil, err
		}
	} else if !os.IsNotExist(e) {
		return nil, e
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

	l.Config = packages.Config{Context: ctx, Dir: root, Env: env, Mode: packages.LoadSyntax | packages.NeedForTest, Tests: true, BuildFlags: flags}
	return l, nil
}

func (l *loadContext) collectEffectiveContext(tags string) error {
	cmd := exec.CommandContext(l.Config.Context, "go", "env", "-json")
	cmd.Dir, cmd.Env = l.Root, l.Config.Env
	data, err := cmd.Output()
	if err != nil {
		return fmt.Errorf("effective Go driver context: %w", err)
	}
	if err := json.Unmarshal(data, &l.goEnv); err != nil {
		return err
	}
	for _, key := range []string{"GOVERSION", "GOOS", "GOARCH", "CGO_ENABLED", "GOROOT", "GOCACHE"} {
		if l.goEnv[key] == "" {
			return fmt.Errorf("effective context missing %s", key)
		}
	}
	normalizedTags := strings.FieldsFunc(tags, func(r rune) bool { return r == ',' || r == ' ' || r == '\t' || r == '\n' })
	slices.Sort(normalizedTags)
	normalizedTags = slices.Compact(normalizedTags)
	c := EffectiveBuildContext{GoVersion: l.goEnv["GOVERSION"], GOOS: l.goEnv["GOOS"], GOARCH: l.goEnv["GOARCH"], CGOEnabled: l.goEnv["CGO_ENABLED"], GOEXPERIMENT: l.goEnv["GOEXPERIMENT"], BuildTags: strings.Join(normalizedTags, ","), ArchitectureFeatures: map[string]string{}, Environment: map[string]string{}, BuildFlags: []string{"-mod=readonly"}}
	for _, key := range []string{"GO386", "GOAMD64", "GOARM", "GOARM64", "GOMIPS", "GOMIPS64", "GOPPC64", "GORISCV64", "GOWASM"} {
		if value := l.goEnv[key]; value != "" {
			c.ArchitectureFeatures[key] = value
			c.Environment[key] = value
		}
	}
	for _, key := range []string{"GOWORK", "GOTOOLCHAIN", "GOPROXY", "GOSUMDB", "GOENV", "GOFLAGS", "GOOS", "GOARCH", "CGO_ENABLED", "GOEXPERIMENT"} {
		c.Environment[key] = l.goEnv[key]
	}
	// `go env` renders GOENV=off as an empty filename. Execution needs the
	// actual protective setting, not an empty value that re-enables defaults.
	for _, entry := range l.Config.Env {
		key, value, _ := strings.Cut(entry, "=")
		if key == "GOENV" {
			c.Environment[key] = value
		}
	}
	if c.BuildTags != "" {
		c.BuildFlags = append(c.BuildFlags, "-tags="+c.BuildTags)
	}
	l.Effective = c
	return nil
}
