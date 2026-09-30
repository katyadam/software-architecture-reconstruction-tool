package com.voyantclair.wala;

import static org.junit.jupiter.api.Assertions.*;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.jar.JarEntry;
import java.util.jar.JarOutputStream;
import javax.tools.ToolProvider;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class WalaCallGraphMainTest {
  @Test void reports_no_entrypoints_as_json_result() throws Exception {
    var result = WalaCallGraphMain.run(Path.of("src/test/resources/fixtures/no-entrypoint"));
    assertEquals("no_entrypoints", result.status());
    assertTrue(result.edges().isEmpty());
  }

  @Test void reports_invalid_source_bindings_as_a_failed_result(@TempDir Path tempDir) throws Exception {
    Path source = tempDir.resolve("broken/Main.java");
    Files.createDirectories(source.getParent());
    Files.writeString(
        source,
        "package broken; import missing.Dependency; public class Main { Dependency value; public static void main(String[] args) {} }");

    var result = WalaCallGraphMain.run(tempDir);

    assertEquals("failed", result.status());
    assertFalse(result.diagnostics().isEmpty());
  }

  @Test void reports_unsupported_method_references_as_a_failed_result(@TempDir Path tempDir)
      throws Exception {
    Path source = tempDir.resolve("modern/Main.java");
    Files.createDirectories(source.getParent());
    Files.writeString(
        source,
        "package modern; import java.util.function.Consumer; public class Main { public static void main(String[] args) { Consumer<String> printer = System.out::println; printer.accept(\"test\"); } }");

    var result = WalaCallGraphMain.run(tempDir);

    assertEquals("failed", result.status());
    assertTrue(result.diagnostics().get(0).contains("UnimplementedError"));
  }

  @Test void analyzes_compiled_method_references_without_using_the_source_frontend(@TempDir Path tempDir)
      throws Exception {
    Path source = tempDir.resolve("Main.java");
    Files.writeString(source, "import java.util.function.Consumer; public class Main { public static void main(String[] args) { Consumer<String> printer = System.out::println; printer.accept(\"test\"); } }");
    Path classesDir = tempDir.resolve("classes");
    Files.createDirectories(classesDir);
    assertEquals(0, ToolProvider.getSystemJavaCompiler().run(null, null, null, "--release", "8", "-d", classesDir.toString(), source.toString()));

    var result = WalaCallGraphMain.runBytecode(List.of(classesDir), List.of());

    assertEquals("ok", result.status());
    assertEquals("cha_bytecode", result.algorithm());
  }

  @Test void parses_the_explicit_high_precision_bytecode_algorithm() {
    assertEquals(
        BytecodeCallGraph.Algorithm.ZERO_ONE_CONTAINER_CFA,
        WalaCallGraphMain.parseBytecodeAlgorithm("zero-one-container-cfa"));
  }
  @Test void resolves_interface_dispatch() throws Exception {
    var root = Path.of(getClass().getResource("/fixtures/dispatch").toURI());
    var result = WalaCallGraphMain.run(root);
    assertEquals("ok", result.status());
    assertTrue(result.edges().stream().anyMatch(e -> e.caller().declaring_type().contains("LMain") && e.callee().declaring_type().contains("LImpl") && e.callee().member_name().equals("run")), result.edges().toString());
  }

  @Test void resolves_a_project_that_depends_on_a_supplied_jar(@TempDir Path tempDir) throws Exception {
    Path dependencyJar = createWorkerJar(tempDir);
    Path sourceRoot = writeProjectSource(tempDir);

    var result = WalaCallGraphMain.run(sourceRoot, List.of(dependencyJar));

    assertEquals("ok", result.status());
    assertTrue(
        result.edges().stream()
            .anyMatch(edge -> edge.caller().declaring_type().contains("Lapp/Main")),
        "The source entry point should remain in the resulting call graph");
  }

  /** Creates a real dependency JAR used to verify WALA resolves external source types. */
  private static Path createWorkerJar(Path tempDir) throws IOException {
    Path dependencySource = tempDir.resolve("dependency-source/library/Worker.java");
    Files.createDirectories(dependencySource.getParent());
    Files.writeString(dependencySource, "package library; public interface Worker { void run(); }");
    Path classes = tempDir.resolve("dependency-classes");
    Files.createDirectories(classes);
    assertEquals(
        0,
        ToolProvider.getSystemJavaCompiler()
            .run(null, null, null, "--release", "8", "-d", classes.toString(), dependencySource.toString()));

    Path jar = tempDir.resolve("worker.jar");
    try (var output = new JarOutputStream(Files.newOutputStream(jar))) {
      output.putNextEntry(new JarEntry("library/Worker.class"));
      output.write(Files.readAllBytes(classes.resolve("library/Worker.class")));
      output.closeEntry();
    }
    return jar;
  }

  /** Writes a project whose interface dispatch requires the external dependency JAR. */
  private static Path writeProjectSource(Path tempDir) throws IOException {
    Path sourceRoot = tempDir.resolve("project-source");
    Path source = sourceRoot.resolve("app/Main.java");
    Files.createDirectories(source.getParent());
    Files.writeString(
        source,
        "package app; import library.Worker; class Impl implements Worker { public void run() {} } public class Main { public static void main(String[] args) { Worker worker = new Impl(); worker.run(); } }");
    return sourceRoot;
  }
}
