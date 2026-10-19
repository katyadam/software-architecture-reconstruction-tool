package com.voyantclair.callgraph;

import java.util.List;

/** Defines the versioned JSON contract shared by independent Java call-graph adapters. */
public record CallGraphResult(
    int schema_version,
    String status,
    String provider_id,
    String source_root,
    String algorithm,
    List<String> diagnostics,
    List<Reachability> reachability,
    List<Edge> edges) {
  /** Identifies a JVM method endpoint without coupling an adapter to a particular analysis tool. */
  public record MethodRef(
      String language,
      String declaring_type,
      String member_name,
      String descriptor,
      String source_path,
      Integer source_line) {}

  /** Records selected entrypoints that can reach one application method. */
  public record Reachability(MethodRef method, List<MethodRef> entrypoints) {}

  /** Records one directed call relation produced by an adapter. */
  public record Edge(
      MethodRef caller,
      MethodRef callee,
      String provider_id,
      String algorithm,
      Float confidence) {}
}
