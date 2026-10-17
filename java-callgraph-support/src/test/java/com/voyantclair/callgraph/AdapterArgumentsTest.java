package com.voyantclair.callgraph;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.nio.file.Path;
import java.util.List;
import org.junit.jupiter.api.Test;

class AdapterArgumentsTest {
  @Test
  void preserves_repeated_selectors_and_bytecode_locations() {
    var arguments =
        AdapterArguments.parse(
            new String[] {
              "--source-dir", "project/src/main/java",
              "--classes-dir", "project/target/classes",
              "--test-classes-dir", "project/target/test-classes",
              "--classpath", "dependencies/first.jar" + java.io.File.pathSeparator + "dependencies/second.jar",
              "--entrypoint", "Lexample/ServiceTest#changes()V",
              "--entrypoint", "Lexample/ServiceTest#other()V",
              "--target", "Lexample/Service#changed()V",
              "--target", "Lexample/Service#other()V",
              "--algorithm", "cha"
            });

    assertEquals(Path.of("project/src/main/java"), arguments.sourceDir());
    assertEquals(List.of(Path.of("project/target/classes")), arguments.applicationClassDirs());
    assertEquals(List.of(Path.of("project/target/test-classes")), arguments.testClassDirs());
    assertEquals(
        List.of(Path.of("dependencies/first.jar"), Path.of("dependencies/second.jar")),
        arguments.classpath());
    assertEquals(
        List.of("Lexample/ServiceTest#changes()V", "Lexample/ServiceTest#other()V"),
        arguments.entrypoints());
    assertEquals(
        List.of("Lexample/Service#changed()V", "Lexample/Service#other()V"),
        arguments.targets());
    assertEquals("cha", arguments.algorithm());
  }

  @Test
  void rejects_a_missing_source_directory() {
    var error =
        assertThrows(
            IllegalArgumentException.class,
            () -> AdapterArguments.parse(new String[] {"--algorithm", "cha"}));

    assertEquals("Missing required --source-dir argument", error.getMessage());
  }
}
