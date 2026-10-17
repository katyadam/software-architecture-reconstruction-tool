package main

import "io"

type MethodRef struct {
	Language      string `json:"language"`
	DeclaringType string `json:"declaring_type"`
	MemberName    string `json:"member_name"`
	Descriptor    string `json:"descriptor"`
	SourcePath    string `json:"source_path"`
	SourceLine    int    `json:"source_line"`
}

type Reachability struct {
	Method      MethodRef   `json:"method"`
	Entrypoints []MethodRef `json:"entrypoints"`
}

type Scope struct {
	DependencyBodies string   `json:"dependency_bodies"`
	GOOS             string   `json:"goos"`
	GOARCH           string   `json:"goarch"`
	GoVersion        string   `json:"go_version"`
	BuildTags        string   `json:"build_tags"`
	WorkspaceMode    string   `json:"workspace_mode"`
	Limitations      []string `json:"limitations"`
}

// Outcome is a targeted graph proof, not a production-safe exclusion decision.
type Outcome struct {
	SchemaVersion int            `json:"schema_version"`
	Status        string         `json:"status"`
	ProviderID    string         `json:"provider_id"`
	SourceRoot    string         `json:"source_root"`
	Algorithm     string         `json:"algorithm"`
	Diagnostics   []string       `json:"diagnostics"`
	Reachability  []Reachability `json:"reachability"`
	Edges         []any          `json:"edges"`
	Tests         []MethodRef    `json:"tests"`
	Scope         Scope          `json:"scope"`
}

type Options struct {
	SourceDir string
	Targets   []string
	Tags      string
	Progress  io.Writer
}

type SourceInput struct {
	SourcePath string `json:"source_path"`
	Source     string `json:"source"`
}
type SourceDeclaration struct {
	MemberName    string `json:"member_name"`
	Header        string `json:"header"`
	StartLine     int    `json:"start_line"`
	EndLine       int    `json:"end_line"`
	BodyStartLine int    `json:"body_start_line"`
	BodyEndLine   int    `json:"body_end_line"`
}
type FileDescription struct {
	SourcePath   string              `json:"source_path"`
	PackageName  *string             `json:"package_name"`
	Declarations []SourceDeclaration `json:"declarations"`
	Diagnostics  []string            `json:"diagnostics"`
}
type DescriptionOutcome struct {
	SchemaVersion int               `json:"schema_version"`
	ProviderID    string            `json:"provider_id"`
	Status        string            `json:"status"`
	Files         []FileDescription `json:"files"`
	Diagnostics   []string          `json:"diagnostics"`
}
type NativeTest struct {
	Method           MethodRef `json:"method"`
	ExecutionPackage string    `json:"execution_package"`
}
type NativeDeclaration struct {
	Method MethodRef         `json:"method"`
	Syntax SourceDeclaration `json:"syntax"`
}
type InventoryOutcome struct {
	SchemaVersion         int                 `json:"schema_version"`
	ProviderID            string              `json:"provider_id"`
	Status                string              `json:"status"`
	SourceRoot            string              `json:"source_root"`
	SourceFiles           []string            `json:"source_files"`
	Tests                 []NativeTest        `json:"tests"`
	Declarations          []NativeDeclaration `json:"declarations"`
	TestInventoryComplete bool                `json:"test_inventory_complete"`
	Scope                 Scope               `json:"scope"`
	Diagnostics           []string            `json:"diagnostics"`
}
