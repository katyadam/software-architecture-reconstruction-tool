package main

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
)

func TestCLIEmitsOnlyJSONAndReportsProgressSeparately(t *testing.T) {
	var stdout, stderr bytes.Buffer
	code := run([]string{"--source-dir", "testdata/control", "--target", "example.com/tia#Changed"}, &stdout, &stderr)
	if code != 0 {
		t.Fatalf("exit=%d: %s", code, stderr.String())
	}
	var out Outcome
	if err := json.Unmarshal(stdout.Bytes(), &out); err != nil {
		t.Fatalf("stdout is not one JSON document: %v: %s", err, stdout.String())
	}
	if out.Status != "ok" || len(out.Reachability) != 1 || len(out.Reachability[0].Entrypoints) != 5 {
		t.Fatalf("bad result: %+v", out)
	}
	if !strings.Contains(stderr.String(), "callgraph_phase=loading") || !strings.Contains(stderr.String(), "callgraph_phase=reachability_ready") {
		t.Fatalf("missing progress: %s", stderr.String())
	}
}

func TestCLIRejectsInvalidArgumentsAndReturnsStructuredTimeout(t *testing.T) {
	for _, args := range [][]string{{}, {"--source-dir", "testdata/control"}, {"--timeout", "bad"}} {
		var stdout, stderr bytes.Buffer
		if code := run(args, &stdout, &stderr); code != 2 || stdout.Len() != 0 {
			t.Fatalf("invalid arguments accepted: %v; stdout=%s", args, stdout.String())
		}
	}
	var stdout, stderr bytes.Buffer
	if code := run([]string{"--source-dir", "testdata/control", "--target", "example.com/tia#Changed", "--timeout", "1ns"}, &stdout, &stderr); code != 1 {
		t.Fatalf("timeout exit=%d", code)
	}
	var out Outcome
	if err := json.Unmarshal(stdout.Bytes(), &out); err != nil || out.Status != "failed" || len(out.Diagnostics) == 0 {
		t.Fatalf("missing structured failure: %s", stdout.String())
	}
}
