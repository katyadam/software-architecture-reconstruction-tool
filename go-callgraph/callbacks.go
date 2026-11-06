package main

import (
	"fmt"
	"sort"

	"golang.org/x/tools/go/callgraph"
	"golang.org/x/tools/go/ssa"
)

// addSortCallbackEdges summarizes only the synchronous comparator callbacks of
// sort.Slice and sort.SliceStable. Their library bodies are opaque in our SSA.
// Keep each edge at its original caller: linking through the shared library
// node would make independent sorting callers reach each other's callbacks.
func addSortCallbackEdges(graph *callgraph.Graph) []string {
	unresolved := map[string]bool{}
	for fn, caller := range graph.Nodes {
		if fn == nil {
			continue
		}
		for _, block := range fn.Blocks {
			for _, instruction := range block.Instrs {
				site, ok := instruction.(ssa.CallInstruction)
				if !ok {
					continue
				}
				call := site.Common()
				callee := call.StaticCallee()
				if callee == nil || callee.Pkg == nil || callee.Pkg.Pkg.Path() != "sort" ||
					(callee.Name() != "Slice" && callee.Name() != "SliceStable") {
					continue
				}
				var callback *ssa.Function
				if len(call.Args) == 2 {
					switch value := call.Args[1].(type) {
					case *ssa.Function:
						callback = value
					case *ssa.MakeClosure:
						callback, _ = value.Fn.(*ssa.Function)
					}
				}
				callbackNode := graph.Nodes[callback]
				if callback == nil || callbackNode == nil {
					unresolved[fmt.Sprintf("unresolved sort.%s callback in %s; callback coverage remains partial", callee.Name(), fn.String())] = true
					continue
				}
				duplicate := false
				for _, edge := range caller.Out {
					if edge.Site == site && edge.Callee == callbackNode {
						duplicate = true
						break
					}
				}
				if !duplicate {
					callgraph.AddEdge(caller, site, callbackNode)
				}
			}
		}
	}
	diagnostics := make([]string, 0, len(unresolved))
	for message := range unresolved {
		diagnostics = append(diagnostics, message)
	}
	sort.Strings(diagnostics)
	return diagnostics
}
