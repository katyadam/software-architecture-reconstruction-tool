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
