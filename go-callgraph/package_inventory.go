package main

import (
	"context"
	"fmt"
	"golang.org/x/tools/go/packages"
	"path/filepath"
	"slices"
	"sort"
	"strings"
	"time"
)

func loadPackageMetadata(l *loadContext) ([]*packages.Package, error) {
	cfg := l.Config
	cfg.Mode = packages.NeedName | packages.NeedFiles | packages.NeedCompiledGoFiles | packages.NeedImports | packages.NeedDeps | packages.NeedModule | packages.NeedForTest
	pkgs, err := packages.Load(&cfg, "./...")
	if err == nil {
		err = packageLoadError(cfg.Context, pkgs)
	}
	return pkgs, err
}

func DiscoverPackages(ctx context.Context, opts Options) PackageInventoryOutcome {
	out := emptyPackageInventory(opts)
	start := time.Now()
	phase := func(name string) {
		if opts.Progress != nil {
			fmt.Fprintf(opts.Progress, "callgraph_phase=package_inventory_%s provider=go-native elapsed_ms=%d\n", name, time.Since(start).Milliseconds())
		}
	}
	phase("loading")
	l, err := prepareLoad(ctx, opts)
	if err != nil {
		out.Diagnostics = append(out.Diagnostics, err.Error())
		return out
	}
	defer l.Close()
	out.SourceRoot = l.Root
	if err = l.collectEffectiveContext(opts.Tags); err != nil {
		out.Diagnostics = append(out.Diagnostics, err.Error())
		return out
	}
	pkgs, err := loadPackageMetadata(l)
	if err != nil {
		failedPackageScan(&out, err)
		return out
	}
	out.Blockers, err = scanPackageBlockers(ctx, l.Root, pkgs)
	if err != nil {
		out.Diagnostics = append(out.Diagnostics, err.Error())
		return out
	}
	nodes := map[string]*packages.Package{}
	packages.Visit(pkgs, nil, func(p *packages.Package) { nodes[p.ID] = p })
	binaries := map[string]*ExecutionPackage{}
	for _, p := range nodes {
		node, err := packageMetadataNode(l, p)
		if err != nil {
			out.Diagnostics = append(out.Diagnostics, err.Error())
			return out
		}
		out.Packages = append(out.Packages, node)
		if len(node.TestFiles) == 0 {
			continue
		}
		execution := p.ForTest
		if execution == "" {
			out.Diagnostics = append(out.Diagnostics, "test-file owner lacks loader test association: "+p.ID)
			return out
		}
		directory, err := filepath.Rel(l.Root, p.Dir)
		if err != nil || !localPath(l.Root, p.Dir) {
			out.Diagnostics = append(out.Diagnostics, "test binary outside root: "+p.ID)
			return out
		}
		binary := binaries[execution]
		if binary == nil {
			binary = &ExecutionPackage{ImportPath: execution, RelativeDirectory: filepath.ToSlash(directory), Roots: []string{}}
			binaries[execution] = binary
		}
		if binary.RelativeDirectory != filepath.ToSlash(directory) {
			out.Diagnostics = append(out.Diagnostics, "conflicting test binary directories: "+execution)
			return out
		}
		binary.Roots = append(binary.Roots, p.ID)
	}
	// External-only tests can use an ordinary production instance. Include its
	// actual ID; never synthesize test variant or generated-main identities.
	for _, binary := range binaries {
		for _, p := range nodes {
			if p.PkgPath == binary.ImportPath && (p.ForTest == binary.ImportPath || p.ForTest == "") && localPath(l.Root, p.Dir) {
				binary.Roots = append(binary.Roots, p.ID)
			}
		}
		augmented := false
		for _, id := range binary.Roots {
			augmented = augmented || nodes[id].PkgPath == binary.ImportPath && nodes[id].ForTest == binary.ImportPath
		}
		binary.Roots = slices.DeleteFunc(binary.Roots, func(id string) bool {
			return augmented && nodes[id].PkgPath == binary.ImportPath && nodes[id].ForTest == ""
		})
		sort.Strings(binary.Roots)
		binary.Roots = slices.Compact(binary.Roots)
		out.ExecutionPackages = append(out.ExecutionPackages, *binary)
	}
	sort.Slice(out.Packages, func(i, j int) bool { return out.Packages[i].ID < out.Packages[j].ID })
	sort.Slice(out.ExecutionPackages, func(i, j int) bool { return out.ExecutionPackages[i].ImportPath < out.ExecutionPackages[j].ImportPath })
	scope := emptyOutcome(opts).Scope
	out.Scope, out.EffectiveBuildContext = &scope, &l.Effective
	out.Status, out.Complete = "ok", true
	phase("ready")
	return out
}

func packageMetadataNode(l *loadContext, p *packages.Package) (PackageNode, error) {
	node := PackageNode{ID: p.ID, ImportPath: p.PkgPath, Imports: []string{}, SourceFiles: []string{}, TestFiles: []string{}, ForTest: p.ForTest}
	for _, dep := range p.Imports {
		node.Imports = append(node.Imports, dep.ID)
	}
	sort.Strings(node.Imports)
	node.Imports = slices.Compact(node.Imports)
	provenance := PackageProvenance{Directory: p.Dir}
	generated := p.Name == "main" && len(p.GoFiles) > 0
	for _, f := range p.GoFiles {
		generated = generated && localPath(l.goEnv["GOCACHE"], f)
	}
	switch {
	case generated:
		associated := false
		for _, dep := range p.Imports {
			associated = associated || dep.ForTest != "" && localPath(l.Root, dep.Dir)
		}
		if !associated {
			return node, fmt.Errorf("generated main lacks source-backed test association: %s", p.ID)
		}
		provenance.Kind = "generated_test_harness"
		provenance.Directory = filepath.Dir(p.GoFiles[0])
	case p.Module != nil:
		m := p.Module
		provenance.ModulePath, provenance.ModuleVersion = m.Path, m.Version
		if m.Replace != nil && m.Replace.Version == "" {
			provenance.Kind, provenance.Directory = "local_replacement", m.Replace.Dir
		} else if m.Main && localPath(l.Root, p.Dir) {
			provenance.Kind = "verified_module"
		} else if m.Version != "" || m.Replace != nil && m.Replace.Version != "" {
			provenance.Kind = "module_cache"
		} else {
			return node, fmt.Errorf("unrecognized module provenance: %s", p.ID)
		}
		if provenance.Kind == "verified_module" || provenance.Kind == "local_replacement" && localPath(l.Root, p.Dir) {
			for _, f := range p.GoFiles {
				canonical, err := filepath.EvalSymlinks(f)
				if err != nil || !localPath(l.Root, canonical) {
					return node, fmt.Errorf("uninspectable local source: %s (package=%s name=%s module=%+v)", f, p.ID, p.Name, p.Module)
				}
				node.SourceFiles = append(node.SourceFiles, canonical)
				if strings.HasSuffix(canonical, "_test.go") {
					node.TestFiles = append(node.TestFiles, canonical)
				}
			}
		}
	case localPath(filepath.Join(l.goEnv["GOROOT"], "src"), p.Dir) || p.PkgPath == "unsafe" && p.Name == "unsafe" && len(p.GoFiles) == 0:
		provenance.Kind = "standard_library"
		if provenance.Directory == "" {
			provenance.Directory = filepath.Join(l.goEnv["GOROOT"], "src", "unsafe")
		}
	default:
		return node, fmt.Errorf("unrecognized package provenance: %s (%s)", p.ID, p.Dir)
	}
	node.Provenance = provenance
	sort.Strings(node.SourceFiles)
	node.SourceFiles = slices.Compact(node.SourceFiles)
	sort.Strings(node.TestFiles)
	node.TestFiles = slices.Compact(node.TestFiles)
	return node, nil
}
