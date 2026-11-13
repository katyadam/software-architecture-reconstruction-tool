package main

type EffectiveBuildContext struct {
	GoVersion            string            `json:"go_version"`
	GOOS                 string            `json:"goos"`
	GOARCH               string            `json:"goarch"`
	CGOEnabled           string            `json:"cgo_enabled"`
	ArchitectureFeatures map[string]string `json:"architecture_features"`
	GOEXPERIMENT         string            `json:"goexperiment"`
	BuildTags            string            `json:"build_tags"`
	Environment          map[string]string `json:"environment"`
	BuildFlags           []string          `json:"build_flags"`
}
type PackageProvenance struct {
	Kind          string `json:"kind"`
	Directory     string `json:"directory"`
	ModulePath    string `json:"module_path,omitempty"`
	ModuleVersion string `json:"module_version,omitempty"`
}
type PackageNode struct {
	ID          string            `json:"id"`
	ImportPath  string            `json:"import_path"`
	Imports     []string          `json:"imports"`
	Provenance  PackageProvenance `json:"provenance"`
	SourceFiles []string          `json:"source_files"`
	TestFiles   []string          `json:"test_files"`
	ForTest     string            `json:"for_test"`
}
type ExecutionPackage struct {
	ImportPath        string   `json:"import_path"`
	RelativeDirectory string   `json:"relative_directory"`
	Roots             []string `json:"roots"`
}
type PackageBlocker struct {
	Code       string `json:"code"`
	SourcePath string `json:"source_path,omitempty"`
	Diagnostic string `json:"diagnostic"`
}
type PackageInventoryOutcome struct {
	SchemaVersion         int                    `json:"schema_version"`
	ProviderID            string                 `json:"provider_id"`
	Status                string                 `json:"status"`
	SourceRoot            string                 `json:"source_root"`
	Scope                 *Scope                 `json:"scope"`
	EffectiveBuildContext *EffectiveBuildContext `json:"effective_build_context"`
	Complete              bool                   `json:"complete"`
	Packages              []PackageNode          `json:"packages"`
	ExecutionPackages     []ExecutionPackage     `json:"execution_packages"`
	Blockers              []PackageBlocker       `json:"blockers"`
	Diagnostics           []string               `json:"diagnostics"`
}

func emptyPackageInventory(opts Options) PackageInventoryOutcome {
	return PackageInventoryOutcome{SchemaVersion: 1, ProviderID: "go-native", Status: "failed", SourceRoot: opts.SourceDir, Packages: []PackageNode{}, ExecutionPackages: []ExecutionPackage{}, Blockers: []PackageBlocker{}, Diagnostics: []string{}}
}
