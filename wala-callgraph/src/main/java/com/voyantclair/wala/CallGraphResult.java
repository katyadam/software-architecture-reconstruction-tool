package com.voyantclair.wala;
import java.util.List;
public record CallGraphResult(int schema_version, String status, String provider_id, String source_root, String algorithm, List<String> diagnostics, List<Edge> edges) {
  public record MethodRef(String language, String declaring_type, String member_name, String descriptor, String source_path, Integer source_line) {}
  public record Edge(MethodRef caller, MethodRef callee, String provider_id, String algorithm, Float confidence) {}
}
