package com.voyantclair.wala;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** Verifies Maven reactor preparation without relying on benchmark-specific layouts. */
class MavenBytecodeArtifactsTest {
  @Test
  void prepares_sibling_module_classes_and_dependency_jars(@TempDir Path tempDir) throws Exception {
    Path sourceRoot = createReactor(tempDir);

    var preparation = MavenBytecodeArtifacts.prepare(sourceRoot);

    assertTrue(preparation.succeeded(), preparation.diagnostic());
    assertTrue(
        preparation.artifacts().applicationClassDirs().stream()
            .anyMatch(path -> path.endsWith("base/target/classes")));
    assertTrue(
        preparation.artifacts().applicationClassDirs().stream()
            .anyMatch(path -> path.endsWith("service/target/classes")));
    assertFalse(preparation.artifacts().dependencyJars().isEmpty());
  }

  @Test
  void returns_a_diagnostic_when_the_maven_build_fails(@TempDir Path tempDir) throws Exception {
    Path sourceRoot = tempDir.resolve("service/src/main/java");
    Files.createDirectories(sourceRoot);
    Files.writeString(tempDir.resolve("service/pom.xml"), "<project>invalid</project>");

    var preparation = MavenBytecodeArtifacts.prepare(sourceRoot);

    assertFalse(preparation.succeeded());
    assertFalse(preparation.diagnostic().isBlank());
  }

  /** Creates a two-module Maven reactor with a real sibling-module dependency. */
  private static Path createReactor(Path root) throws IOException {
    String version = "1.0-" + UUID.randomUUID();
    Files.writeString(
        root.resolve("pom.xml"),
        "<project xmlns=\"http://maven.apache.org/POM/4.0.0\"><modelVersion>4.0.0</modelVersion><groupId>fixture</groupId><artifactId>parent</artifactId><version>"
            + version
            + "</version><packaging>pom</packaging><properties><maven.compiler.release>8</maven.compiler.release></properties><modules><module>base</module><module>service</module></modules></project>");
    writeModulePom(root.resolve("base"), "base", version, null);
    writeModulePom(root.resolve("service"), "service", version, "base");
    writeJava(root.resolve("base/src/main/java/fixture/Base.java"), "package fixture; public class Base { public static void work() {} }");
    writeJava(root.resolve("service/src/main/java/fixture/Main.java"), "package fixture; public class Main { public static void main(String[] args) { Base.work(); } }");
    return root.resolve("service/src/main/java");
  }

  /** Writes one child POM with an optional dependency on the fixture's base module. */
  private static void writeModulePom(Path module, String artifact, String version, String dependency)
      throws IOException {
    Files.createDirectories(module);
    String dependencyXml =
        dependency == null
            ? ""
            : "<dependencies><dependency><groupId>fixture</groupId><artifactId>"
                + dependency
                + "</artifactId><version>"
                + version
                + "</version></dependency></dependencies>";
    Files.writeString(
        module.resolve("pom.xml"),
        "<project xmlns=\"http://maven.apache.org/POM/4.0.0\"><modelVersion>4.0.0</modelVersion><parent><groupId>fixture</groupId><artifactId>parent</artifactId><version>"
            + version
            + "</version></parent><artifactId>"
            + artifact
            + "</artifactId>"
            + dependencyXml
            + "</project>");
  }

  /** Writes a Java source file and creates its parent directory. */
  private static void writeJava(Path source, String code) throws IOException {
    Files.createDirectories(source.getParent());
    Files.writeString(source, code);
  }
}
