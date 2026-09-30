package com.voyantclair.wala;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import javax.tools.ToolProvider;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** Verifies WALA binary analysis against real Java class files. */
class BytecodeCallGraphTest {
  @Test
  void resolves_interface_dispatch_from_compiled_application_classes(@TempDir Path tempDir)
      throws Exception {
    Path classesDir = compileDispatchFixture(tempDir);

    var result = BytecodeCallGraph.analyze(List.of(classesDir), List.of());

    assertEquals("ok", result.status());
    assertTrue(
        result.edges().stream()
            .anyMatch(
                edge ->
                    edge.caller().declaring_type().equals("LMain")
                        && edge.callee().declaring_type().equals("LImpl")
                        && edge.callee().member_name().equals("run")),
        result.edges().toString());
  }

  @Test
  void emits_only_edges_with_application_callers(@TempDir Path tempDir) throws Exception {
    Path classesDir = compileDispatchFixture(tempDir);

    var result = BytecodeCallGraph.analyze(List.of(classesDir), List.of());

    assertEquals("ok", result.status());
    assertFalse(result.edges().isEmpty());
    assertTrue(
        result.edges().stream()
            .allMatch(edge -> !edge.caller().declaring_type().startsWith("Ljava/")),
        result.edges().toString());
  }

  /** Compiles a Java 8 fixture whose virtual dispatch has a known concrete target. */
  private static Path compileDispatchFixture(Path tempDir) throws IOException {
    Path source = tempDir.resolve("Main.java");
    Files.writeString(
        source,
        "interface Worker { void run(); } class Impl implements Worker { public void run() {} } public class Main { public static void main(String[] args) { Worker worker = new Impl(); worker.run(); } }");
    Path classesDir = tempDir.resolve("classes");
    Files.createDirectories(classesDir);
    assertEquals(
        0,
        ToolProvider.getSystemJavaCompiler()
            .run(null, null, null, "--release", "8", "-d", classesDir.toString(), source.toString()));
    return classesDir;
  }
}
