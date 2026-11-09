package com.voyantclair.spoon;

import static org.junit.jupiter.api.Assertions.*;

import com.voyantclair.callgraph.CallGraphResult;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

class SpoonCallGraphTest {

  @Test
  void analyzes_simple_source_directory_without_classpath() throws Exception {
    var root = Path.of(getClass().getResource("/fixtures/dispatch").toURI());
    var result = SpoonCallGraph.analyze(List.of(root), List.of(), List.of(), List.of());

    assertEquals("ok", result.status());
    assertEquals(SpoonCallGraph.PROVIDER_ID, result.provider_id());
    assertEquals(SpoonCallGraph.ALGORITHM, result.algorithm());
    assertFalse(result.edges().isEmpty(), "Expected at least one call edge");
  }

  @Test
  void records_main_to_run_call_edge() throws Exception {
    var root = Path.of(getClass().getResource("/fixtures/dispatch").toURI());
    var result = SpoonCallGraph.analyze(List.of(root), List.of(), List.of(), List.of());

    boolean hasMainToRun = result.edges().stream().anyMatch(e ->
        e.caller().declaring_type().contains("Main")
        && e.callee().member_name().equals("run"));
    assertTrue(hasMainToRun, "Expected Main -> run() edge; edges: " + result.edges());
  }

  @Test
  void returns_failed_status_for_empty_directory(@TempDir Path tempDir) {
    // Empty dir has no Java files — Spoon returns ok with zero edges, not failed.
    var result = SpoonCallGraph.analyze(List.of(tempDir), List.of(), List.of(), List.of());
    assertEquals("ok", result.status());
    assertTrue(result.edges().isEmpty());
  }

  @Test
  void computes_reachability_from_entrypoint_to_changed_target(@TempDir Path tempDir) throws Exception {
    // Write a two-method chain: Test.testA() -> Service.changed()
    Path src = tempDir.resolve("example/ServiceTest.java");
    Files.createDirectories(src.getParent());
    Files.writeString(src, """
        package example;
        class Service { void changed() {} }
        class ServiceTest {
            void testA() { new Service().changed(); }
        }
        """);

    var entrypoint = new SpoonCallGraph.MethodSelector(
        "Lexample/ServiceTest", "testA", "()V");
    var target = new SpoonCallGraph.TargetSelector(
        "Lexample/Service", "changed", 0);

    var result = SpoonCallGraph.analyze(List.of(tempDir), List.of(), List.of(entrypoint), List.of(target));

    assertEquals("ok", result.status());
    boolean testReachesChanged = result.reachability().stream().anyMatch(r ->
        r.method().member_name().equals("changed")
        && r.entrypoints().stream().anyMatch(ep -> ep.member_name().equals("testA")));
    assertTrue(testReachesChanged, "Expected changed() reachable from testA(); reachability: " + result.reachability());
  }

  @Test
  void includes_analysis_duration_diagnostic() throws Exception {
    var root = Path.of(getClass().getResource("/fixtures/dispatch").toURI());
    var result = SpoonCallGraph.analyze(List.of(root), List.of(), List.of(), List.of());

    boolean hasDuration = result.diagnostics().stream()
        .anyMatch(d -> d.startsWith("analysis_duration_ms="));
    assertTrue(hasDuration, "Expected analysis_duration_ms diagnostic; got: " + result.diagnostics());
  }

  @Test
  void binary_name_converts_top_level_class() {
    assertEquals("Lcom/example/Service", SpoonCallGraph.binaryName("com.example.Service"));
  }

  @Test
  void binary_name_converts_nested_class() {
    assertEquals("Lcom/example/Outer$Inner", SpoonCallGraph.binaryName("com.example.Outer.Inner"));
  }

  @Test
  void binary_name_converts_default_package_class() {
    assertEquals("LMain", SpoonCallGraph.binaryName("Main"));
  }

  @Test
  void parse_method_selector_extracts_parts() {
    var selector = SpoonCallGraphMain.parseMethodSelector("Lexample/ServiceTest#changesService()V");
    assertEquals("Lexample/ServiceTest", selector.declaringType());
    assertEquals("changesService", selector.memberName());
    assertEquals("()V", selector.descriptor());
  }

  @Test
  void parse_target_selector_extracts_parts() {
    var selector = SpoonCallGraphMain.parseTargetSelector("Lexample/Service#changed/0");
    assertEquals("Lexample/Service", selector.declaringType());
    assertEquals("changed", selector.memberName());
    assertEquals(0, selector.parameterCount());
  }

  @Test
  void schema_version_is_two() throws Exception {
    var root = Path.of(getClass().getResource("/fixtures/dispatch").toURI());
    var result = SpoonCallGraph.analyze(List.of(root), List.of(), List.of(), List.of());
    assertEquals(2, result.schema_version());
  }
}
