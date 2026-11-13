package com.voyantclair.wala;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.List;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** Verifies Maven reactor preparation without relying on benchmark-specific layouts. */
class MavenBytecodeArtifactsTest {
  @Test
  void uses_configured_java_home_only_for_maven_subprocess() {
    var environment = new HashMap<String, String>();
    environment.put("JAVA_HOME", "/adapter-jdk");

    MavenBytecodeArtifacts.configureMavenJavaHome(environment, "/benchmark-jdk");

    assertEquals("/benchmark-jdk", environment.get("JAVA_HOME"));
  }

  @Test
  void preserves_maven_java_home_when_no_override_is_configured() {
    var environment = new HashMap<String, String>();
    environment.put("JAVA_HOME", "/adapter-jdk");

    MavenBytecodeArtifacts.configureMavenJavaHome(environment, " ");

    assertEquals("/adapter-jdk", environment.get("JAVA_HOME"));
  }

  @Test
  void prepares_only_selected_module_classes_and_dependency_jars(@TempDir Path tempDir)
      throws Exception {
    Path sourceRoot = createReactor(tempDir);

    var preparation = new MavenBytecodeArtifacts().prepare(sourceRoot);

    assertTrue(preparation.succeeded(), preparation.diagnostic());
    BytecodeArtifacts.PreparedArtifacts artifacts = preparation.artifacts();
    assertTrue(artifacts.moduleRoot().endsWith("service"));
    assertEquals(
        List.of(tempDir.resolve("service/target/classes")), artifacts.applicationClassDirs());
    assertEquals(
        List.of(tempDir.resolve("service/target/test-classes")), artifacts.testClassDirs());
    assertFalse(artifacts.dependencyJars().isEmpty());
    assertFalse(
        Files.exists(
            sourceRoot
                .getParent()
                .getParent()
                .getParent()
                .resolve("target/failsafe-reports")),
        "WALA preparation must not run Failsafe integration tests");
    assertFalse(
        Files.exists(sourceRoot.getParent().getParent().getParent().resolve("target/wala-m2")),
        "WALA preparation must reuse Maven's configured repository");
  }

  @Test
  void returns_a_diagnostic_when_the_maven_build_fails(@TempDir Path tempDir) throws Exception {
    Path sourceRoot = tempDir.resolve("service/src/main/java");
    Files.createDirectories(sourceRoot);
    Files.writeString(tempDir.resolve("service/pom.xml"), "<project>invalid</project>");

    var preparation = new MavenBytecodeArtifacts().prepare(sourceRoot);

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
            + "</version><packaging>pom</packaging><properties><maven.compiler.release>8</maven.compiler.release><skipITs>false</skipITs></properties><build><plugins><plugin><groupId>org.apache.maven.plugins</groupId><artifactId>maven-failsafe-plugin</artifactId><version>3.5.2</version><configuration><skipTests>${skipITs}</skipTests></configuration><executions><execution><goals><goal>integration-test</goal><goal>verify</goal></goals></execution></executions></plugin></plugins></build><modules><module>base</module><module>service</module></modules></project>");
    writeModulePom(root.resolve("base"), "base", version, null);
    writeModulePom(root.resolve("service"), "service", version, "base");
    writeJava(root.resolve("base/src/main/java/fixture/Base.java"), "package fixture; public class Base { public static void work() {} }");
    writeJava(root.resolve("base/src/test/java/fixture/BaseFixture.java"), "package fixture; public class BaseFixture {}");
    writeJava(root.resolve("service/src/main/java/fixture/Main.java"), "package fixture; public class Main { public static void main(String[] args) { Base.work(); } }");
    writeJava(root.resolve("service/src/test/java/fixture/MainTest.java"), "package fixture; public class MainTest { public void testMain() { Main.main(new String[0]); } }");
    writeJava(
        root.resolve("service/src/test/java/fixture/FailingIT.java"),
        "package fixture; import org.junit.jupiter.api.Test; import static org.junit.jupiter.api.Assertions.fail; class FailingIT { @Test void fails_if_failsafe_runs() { fail(\"Failsafe must not run during WALA preparation\"); } }");
    return root.resolve("service/src/main/java");
  }

  /** Writes one child POM with an optional dependency on the fixture's base module. */
  private static void writeModulePom(Path module, String artifact, String version, String dependency)
      throws IOException {
    Files.createDirectories(module);
    String dependencyXml =
        "<dependencies>"
            + (dependency == null
                ? ""
                : "<dependency><groupId>fixture</groupId><artifactId>"
                    + dependency
                    + "</artifactId><version>"
                    + version
                    + "</version></dependency><dependency><groupId>fixture</groupId><artifactId>"
                    + dependency
                    + "</artifactId><version>"
                    + version
                    + "</version><classifier>tests</classifier><scope>test</scope></dependency>")
            + "<dependency><groupId>org.junit.jupiter</groupId><artifactId>junit-jupiter</artifactId><version>5.11.4</version><scope>test</scope></dependency></dependencies>";
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
