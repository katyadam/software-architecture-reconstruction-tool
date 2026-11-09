package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"os"
	"sync"
	"time"
)

type targetFlags []string

func (f *targetFlags) String() string         { return fmt.Sprint([]string(*f)) }
func (f *targetFlags) Set(value string) error { *f = append(*f, value); return nil }

// The worker may still be inside SSA when its deadline expires. Stop its
// progress writes before returning; main exits to terminate the worker itself.
type progressWriter struct {
	mu          sync.Mutex
	destination io.Writer
}

func (w *progressWriter) Write(p []byte) (int, error) {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.destination == nil {
		return len(p), nil
	}
	return w.destination.Write(p)
}
func (w *progressWriter) close() { w.mu.Lock(); defer w.mu.Unlock(); w.destination = nil }

func run(args []string, stdout, stderr io.Writer) int {
	return runInput(args, os.Stdin, stdout, stderr)
}

func runInput(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("go-callgraph", flag.ContinueOnError)
	flags.SetOutput(stderr)
	var opts Options
	var targets targetFlags
	mode := flags.String("mode", "graph", "graph, inventory, package-inventory, or describe")
	flags.StringVar(&opts.SourceDir, "source-dir", "", "Go module root (required)")
	flags.StringVar(&opts.Tags, "tags", "", "comma-separated build tags")
	flags.Var(&targets, "target", "native package#function or package#(Receiver).Method; repeatable")
	timeout := flags.Duration("timeout", 300*time.Second, "analysis deadline (e.g. 5m)")
	if err := flags.Parse(args); err != nil {
		return 2
	}
	if (*mode != "graph" && *mode != "inventory" && *mode != "package-inventory" && *mode != "describe") || (*mode != "describe" && opts.SourceDir == "") || (*mode == "graph" && len(targets) == 0) || flags.NArg() != 0 || *timeout <= 0 {
		fmt.Fprintln(stderr, "--source-dir, at least one --target, and a positive --timeout are required")
		return 2
	}
	opts.Targets = targets
	progress := &progressWriter{destination: stderr}
	defer progress.close()
	opts.Progress = progress
	ctx, cancel := context.WithTimeout(context.Background(), *timeout)
	defer cancel()
	type response struct {
		value  any
		failed bool
	}
	result := make(chan response, 1)
	go func() {
		switch *mode {
		case "package-inventory":
			out := DiscoverPackages(ctx, opts)
			result <- response{out, out.Status == "failed"}
		case "inventory":
			out := Discover(ctx, opts)
			result <- response{out, out.Status == "failed"}
		case "describe":
			var input struct {
				Files []SourceInput `json:"files"`
			}
			if err := json.NewDecoder(stdin).Decode(&input); err != nil {
				result <- response{DescriptionOutcome{SchemaVersion: 1, ProviderID: "go-native", Status: "failed", Files: []FileDescription{}, Diagnostics: []string{err.Error()}}, true}
				return
			}
			out := DescribeSources(ctx, input.Files)
			result <- response{out, out.Status == "failed"}
		default:
			out := Analyze(ctx, opts)
			result <- response{out, out.Status == "failed"}
		}
	}()
	var out response
	select {
	case out = <-result:
	case <-ctx.Done():
		message := fmt.Sprintf("Go %s deadline exceeded after %s", *mode, *timeout)
		switch *mode {
		case "package-inventory":
			failed := emptyPackageInventory(opts)
			failed.Diagnostics = append(failed.Diagnostics, message)
			out = response{failed, true}
		case "inventory":
			out = response{InventoryOutcome{SchemaVersion: 1, ProviderID: "go-native", Status: "failed", SourceRoot: opts.SourceDir, Tests: []NativeTest{}, SourceFiles: []string{}, Declarations: []NativeDeclaration{}, Scope: emptyOutcome(opts).Scope, Diagnostics: []string{message}}, true}
		case "describe":
			out = response{DescriptionOutcome{SchemaVersion: 1, ProviderID: "go-native", Status: "failed", Files: []FileDescription{}, Diagnostics: []string{message}}, true}
		default:
			failed := emptyOutcome(opts)
			failed.Diagnostics = append(failed.Diagnostics, message)
			out = response{failed, true}
		}
	}
	if err := json.NewEncoder(stdout).Encode(out.value); err != nil {
		fmt.Fprintln(stderr, err)
		return 1
	}
	if out.failed {
		return 1
	}
	return 0
}

func main() { os.Exit(run(os.Args[1:], os.Stdout, os.Stderr)) }
