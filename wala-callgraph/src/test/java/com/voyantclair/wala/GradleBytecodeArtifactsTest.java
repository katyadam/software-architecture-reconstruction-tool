package com.voyantclair.wala;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** Verifies Gradle bytecode preparation through conventional Gradle project layouts. */
class GradleBytecodeArtifactsTest {
  @Test
  void reports_a_diagnostic_when_no_gradle_project_owns_the_source_root(@TempDir Path tempDir)
      throws Exception {
    Path sourceRoot = tempDir.resolve("src/main/java");
    Files.createDirectories(sourceRoot);

    var preparation = new GradleBytecodeArtifacts().prepare(sourceRoot);

    assertFalse(preparation.succeeded());
    assertTrue(preparation.diagnostic().contains("Gradle"));
  }

  @Test
  void prepares_groovy_gradle_wrapper_outputs(@TempDir Path tempDir) throws Exception {
    Path module = tempDir.resolve("service");
    Path sourceRoot = module.resolve("src/main/java");
    Path mainClasses = module.resolve("build/classes/java/main");
    Path testClasses = module.resolve("build/classes/java/test");
    Path dependency = module.resolve("libs/test.jar");
    Files.createDirectories(sourceRoot);
    Files.createDirectories(mainClasses);
    Files.createDirectories(testClasses);
    Files.createDirectories(dependency.getParent());
    Files.createFile(dependency);
    Files.writeString(module.resolve("build.gradle"), "plugins { id 'java' }");
    Path wrapper = tempDir.resolve("gradlew");
    Path arguments = tempDir.resolve("gradle-arguments.txt");
    Files.writeString(
        wrapper,
        "#!/bin/sh\necho \"$@\" >> " + arguments + "\necho \"VOYANTCLAIR_MAIN=" + mainClasses + "\"\necho \"VOYANTCLAIR_TEST=" + testClasses + "\"\necho \"VOYANTCLAIR_TEST_RUNTIME=" + dependency + "\"\n");
    wrapper.toFile().setExecutable(true);

    var preparation = new GradleBytecodeArtifacts().prepare(sourceRoot);

    assertTrue(preparation.succeeded(), preparation.diagnostic());
    assertEquals(List.of(mainClasses), preparation.artifacts().applicationClassDirs());
    assertEquals(List.of(testClasses), preparation.artifacts().testClassDirs());
    assertEquals(List.of(dependency), preparation.artifacts().dependencyJars());
    assertTrue(Files.readString(arguments).contains(":service:testClasses"));
    assertTrue(Files.readString(arguments).contains("-I"));
  }

  @Test
  void prepares_a_nested_kotlin_dsl_module_with_its_gradle_project_path(@TempDir Path tempDir)
      throws Exception {
    Path module = tempDir.resolve("services/orders");
    Path sourceRoot = module.resolve("src/main/java");
    Path mainClasses = module.resolve("build/classes/java/main");
    Path testClasses = module.resolve("build/classes/java/test");
    Path arguments = tempDir.resolve("nested-arguments.txt");
    Files.createDirectories(sourceRoot);
    Files.createDirectories(mainClasses);
    Files.createDirectories(testClasses);
    Files.writeString(tempDir.resolve("settings.gradle.kts"), "rootProject.name = \"fixture\"");
    Files.writeString(module.resolve("build.gradle.kts"), "plugins { java }");
    Path wrapper = tempDir.resolve("gradlew");
    Files.writeString(
        wrapper,
        "#!/bin/sh\necho \"$@\" >> " + arguments + "\necho \"VOYANTCLAIR_MAIN=" + mainClasses + "\"\necho \"VOYANTCLAIR_TEST=" + testClasses + "\"\necho \"VOYANTCLAIR_TEST_RUNTIME=\"\n");
    wrapper.toFile().setExecutable(true);

    var preparation = new GradleBytecodeArtifacts().prepare(sourceRoot);

    assertTrue(preparation.succeeded(), preparation.diagnostic());
    assertTrue(Files.readString(arguments).contains(":services:orders:testClasses"));
  }

  @Test
  void uses_configured_system_gradle_when_no_wrapper_exists(@TempDir Path tempDir) throws Exception {
    Path sourceRoot = tempDir.resolve("src/main/java");
    Path mainClasses = tempDir.resolve("build/classes/java/main");
    Path testClasses = tempDir.resolve("build/classes/java/test");
    Files.createDirectories(sourceRoot);
    Files.createDirectories(mainClasses);
    Files.createDirectories(testClasses);
    Files.writeString(tempDir.resolve("build.gradle"), "plugins { id 'java' }");
    Path fakeGradle = tempDir.resolve("fake-gradle");
    Files.writeString(fakeGradle, "#!/bin/sh\necho \"VOYANTCLAIR_MAIN=" + mainClasses + "\"\necho \"VOYANTCLAIR_TEST=" + testClasses + "\"\necho \"VOYANTCLAIR_TEST_RUNTIME=\"\n");
    fakeGradle.toFile().setExecutable(true);
    System.setProperty("voyantclair.gradle.command", fakeGradle.toString());
    try {
      assertTrue(new GradleBytecodeArtifacts().prepare(sourceRoot).succeeded());
    } finally {
      System.clearProperty("voyantclair.gradle.command");
    }
  }
}
