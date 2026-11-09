package com.voyantclair.wala;
import java.util.List;

/** Defines the versioned JSON data contract emitted by the standalone WALA adapter. */
public record CallGraphResult(int schema_version, String status, String provider_id, String source_root, String algorithm, List<String> diagnostics, List<Reachability> reachability, List<Edge> edges) {
  /** Identifies a method endpoint without coupling the adapter to Rust model types. */
  public record MethodRef(String language, String declaring_type, String member_name, String descriptor, String source_path, Integer source_line) {}
  /** Records the selected test entrypoints that reach one application method. */
  public record Reachability(MethodRef method, List<MethodRef> entrypoints) {}
  /** Records one directed call relation reported by WALA. */
  public record Edge(MethodRef caller, MethodRef callee, String provider_id, String algorithm, Float confidence) {}
}
