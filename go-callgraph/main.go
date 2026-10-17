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
	flags := flag.NewFlagSet("go-callgraph", flag.ContinueOnError)
	flags.SetOutput(stderr)
	var opts Options
	var targets targetFlags
	flags.StringVar(&opts.SourceDir, "source-dir", "", "Go module root (required)")
	flags.StringVar(&opts.Tags, "tags", "", "comma-separated build tags")
	flags.Var(&targets, "target", "native package#function or package#(Receiver).Method; repeatable")
	timeout := flags.Duration("timeout", 300*time.Second, "analysis deadline (e.g. 5m)")
	if err := flags.Parse(args); err != nil {
		return 2
	}
	if opts.SourceDir == "" || len(targets) == 0 || flags.NArg() != 0 || *timeout <= 0 {
		fmt.Fprintln(stderr, "--source-dir, at least one --target, and a positive --timeout are required")
		return 2
	}
	opts.Targets = targets
	progress := &progressWriter{destination: stderr}
	defer progress.close()
	opts.Progress = progress
	ctx, cancel := context.WithTimeout(context.Background(), *timeout)
	defer cancel()
	result := make(chan Outcome, 1)
	go func() { result <- Analyze(ctx, opts) }()
	var out Outcome
	select {
	case out = <-result:
	case <-ctx.Done():
		out = emptyOutcome(opts)
		out.Diagnostics = append(out.Diagnostics, fmt.Sprintf("Go analysis deadline exceeded after %s", *timeout))
	}
	if err := json.NewEncoder(stdout).Encode(out); err != nil {
		fmt.Fprintln(stderr, err)
		return 1
	}
	if out.Status == "failed" {
		return 1
	}
	return 0
}

func main() { os.Exit(run(os.Args[1:], os.Stdout, os.Stderr)) }
