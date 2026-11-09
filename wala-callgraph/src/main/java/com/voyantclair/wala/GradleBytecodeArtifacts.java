package com.voyantclair.wala;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** Prepares Gradle Java bytecode inputs for the existing WALA binary backend. */
public final class GradleBytecodeArtifacts implements BytecodeArtifactPreparer {
  /** Returns a diagnostic until a conventional Gradle module can be prepared. */
  @Override
  public BytecodeArtifacts.Preparation prepare(Path sourceRoot) {
    Path moduleRoot = findModuleRoot(sourceRoot);
    if (moduleRoot == null) {
      return failure("No conventional Gradle module found for " + sourceRoot);
    }
    Path gradleRoot = findGradleRoot(moduleRoot);
    Path wrapper = gradleRoot.resolve("gradlew");
    Path command =
        Files.isExecutable(wrapper)
            ? wrapper
            : Path.of(System.getProperty("voyantclair.gradle.command", "gradle"));
    try {
      List<String> configuredArguments = configuredArguments();
      Path initScript = Files.createTempFile("voyantclair-gradle-artifacts-", ".gradle");
      Files.writeString(initScript, artifactInitScript());
      try {
        ProcessResult discovery =
            run(
                command,
                gradleRoot,
                withConfiguredArguments(
                    configuredArguments,
                    "-q",
                    "-I",
                    initScript.toString(),
                    "voyantclairWalaProjectPaths"));
        if (discovery.exitCode() != 0) return failure(discovery.output());
        String projectPath =
            discoveredProjectPath(discovery.output(), moduleRoot)
                .orElseGet(() -> projectPath(gradleRoot, moduleRoot));
        ProcessResult compilation =
            run(
                command,
                gradleRoot,
                withConfiguredArguments(configuredArguments, projectPath + ":testClasses"));
        if (compilation.exitCode() != 0) return failure(compilation.output());
        ProcessResult artifacts =
            run(
                command,
                gradleRoot,
                withConfiguredArguments(
                    configuredArguments,
                    "-q",
                    "-I",
                    initScript.toString(),
                    projectPath + ":voyantclairWalaArtifacts"));
        if (artifacts.exitCode() != 0) return failure(artifacts.output());
        String output = artifacts.output();
        var main = paths(output, "VOYANTCLAIR_MAIN=", true);
        var test = paths(output, "VOYANTCLAIR_TEST=", true);
        var dependencies = paths(output, "VOYANTCLAIR_TEST_RUNTIME=", false);
        if (main.isEmpty() || test.isEmpty()) {
          return failure("Gradle did not report Java class directories");
        }
        return new BytecodeArtifacts.Preparation(
            new BytecodeArtifacts.PreparedArtifacts(main, test, dependencies, moduleRoot), "");
      } finally {
        Files.deleteIfExists(initScript);
      }
    } catch (Exception error) {
      return failure(error.toString());
    }
  }

  /** Returns whitespace-separated Gradle flags configured for benchmark-specific build options. */
  private static List<String> configuredArguments() {
    String value = System.getProperty("voyantclair.gradle.arguments");
    if (value == null || value.isBlank()) value = System.getenv("WALA_GRADLE_ARGUMENTS");
    return value == null || value.isBlank() ? List.of() : List.of(value.trim().split("\\s+"));
  }

  /** Prefixes one Gradle invocation with the shared configured options. */
  private static List<String> withConfiguredArguments(List<String> configured, String... arguments) {
    var invocation = new ArrayList<>(configured);
    invocation.addAll(List.of(arguments));
    return invocation;
  }

  /** Returns whether the source root belongs to a conventional Gradle Java module. */
  public static boolean owns(Path sourceRoot) {
    return findModuleRoot(sourceRoot) != null;
  }

  /** Supplies a DSL-independent Gradle task that prints Java source-set artifacts for one project. */
  private static String artifactInitScript() {
    return "gradle.projectsLoaded { rootProject.tasks.register('voyantclairWalaProjectPaths') { doLast { rootProject.allprojects.each { project -> println 'VOYANTCLAIR_PROJECT=' + project.path + '\\t' + project.projectDir } } }; rootProject.allprojects { project -> project.tasks.register('voyantclairWalaArtifacts') { doLast { def sets = project.extensions.findByName('sourceSets'); if (sets != null) { println 'VOYANTCLAIR_MAIN=' + sets.main.output.classesDirs.asPath; println 'VOYANTCLAIR_TEST=' + sets.test.output.classesDirs.asPath; println 'VOYANTCLAIR_TEST_RUNTIME=' + project.configurations.testRuntimeClasspath.asPath } } } } }";
  }

  /** Resolves a selected module directory to its project path from Gradle's evaluated project map. */
  private static java.util.Optional<String> discoveredProjectPath(String output, Path moduleRoot) {
    Path expected = moduleRoot.toAbsolutePath().normalize();
    for (String line : output.lines().toList()) {
      if (!line.startsWith("VOYANTCLAIR_PROJECT=")) continue;
      String[] fields = line.substring("VOYANTCLAIR_PROJECT=".length()).split("\\t", 2);
      if (fields.length == 2 && Path.of(fields[1]).toAbsolutePath().normalize().equals(expected)) {
        return java.util.Optional.of(fields[0]);
      }
    }
    return java.util.Optional.empty();
  }

  /** Converts a Gradle module's path below the root into its colon-prefixed project path. */
  private static String projectPath(Path gradleRoot, Path moduleRoot) {
    Path relative = gradleRoot.relativize(moduleRoot);
    return relative.getNameCount() == 0 ? "" : ":" + relative.toString().replace(java.io.File.separator, ":");
  }

  /** Executes one Gradle wrapper command and captures its merged process output. */
  private static ProcessResult run(Path command, Path root, List<String> arguments) throws Exception {
    var invocation = new ArrayList<String>();
    invocation.add(command.toString());
    invocation.addAll(arguments);
    var process = new ProcessBuilder(invocation).directory(root.toFile()).redirectErrorStream(true).start();
    String output = new String(process.getInputStream().readAllBytes());
    return new ProcessResult(process.waitFor(), output);
  }

  /** Finds the closest enclosing root that supplies the Gradle wrapper command. */
  private static Path findGradleRoot(Path moduleRoot) {
    for (Path current = moduleRoot; current != null; current = current.getParent()) {
      if (Files.isExecutable(current.resolve("gradlew"))) {
        return current;
      }
    }
    return moduleRoot;
  }

  /** Parses one path-separated artifact record and retains only existing expected filesystem entries. */
  private static List<Path> paths(String output, String prefix, boolean directories) {
    var result = new ArrayList<Path>();
    for (String line : output.lines().toList()) {
      if (!line.startsWith(prefix)) continue;
      for (String value : line.substring(prefix.length()).split(java.util.regex.Pattern.quote(java.io.File.pathSeparator))) {
        Path path = Path.of(value);
        if ((directories && Files.isDirectory(path)) || (!directories && Files.isRegularFile(path))) result.add(path);
      }
    }
    return result;
  }

  /** Finds the source-owning directory when it contains either supported Gradle build-script name. */
  private static Path findModuleRoot(Path sourceRoot) {
    Path absolute = sourceRoot.toAbsolutePath().normalize();
    Path main = absolute.getParent();
    Path source = main == null ? null : main.getParent();
    Path module = source == null ? null : source.getParent();
    if (main == null
        || source == null
        || module == null
        || !absolute.getFileName().toString().equals("java")
        || !main.getFileName().toString().equals("main")
        || !source.getFileName().toString().equals("src")) {
      return null;
    }
    return Files.isRegularFile(module.resolve("build.gradle"))
            || Files.isRegularFile(module.resolve("build.gradle.kts"))
        ? module
        : null;
  }

  /** Creates a failed preparation result suitable for WALA's conservative fallback path. */
  private static BytecodeArtifacts.Preparation failure(String diagnostic) {
    return new BytecodeArtifacts.Preparation(null, diagnostic);
  }

  /** Captures one Gradle process completion without throwing adapter exceptions. */
  private record ProcessResult(int exitCode, String output) {}
}
