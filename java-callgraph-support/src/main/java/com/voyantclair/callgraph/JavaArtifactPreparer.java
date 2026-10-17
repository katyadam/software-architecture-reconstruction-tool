package com.voyantclair.callgraph;

import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** Prepares Java bytecode without loading a call-graph library. */
public final class JavaArtifactPreparer {
  private JavaArtifactPreparer() {}

  public static PreparedJavaArtifacts prepare(Path sourceRoot) {
    return Files.isRegularFile(moduleRoot(sourceRoot).resolve("pom.xml"))
        ? maven(sourceRoot)
        : gradle(sourceRoot);
  }

  private static PreparedJavaArtifacts maven(Path sourceRoot) {
    Path module = moduleRoot(sourceRoot);
    if (!Files.isRegularFile(module.resolve("pom.xml"))) {
      return PreparedJavaArtifacts.failure("No conventional Maven module POM found for " + sourceRoot);
    }
    Path reactor = reactorPom(module.resolve("pom.xml"));
    String selector = reactor.getParent().relativize(module).toString();
    ProcessResult build = run("mvn", reactor.getParent(), List.of("-q", "-f", reactor.toString(), "-pl", selector, "-am", "test-compile", "jar:jar", "jar:test-jar", "install:install", "-DskipTests"));
    if (build.exitCode != 0) return PreparedJavaArtifacts.failure(build.output);
    ProcessResult dependencies = run("mvn", module, List.of("-q", "-f", module.resolve("pom.xml").toString(), "-DincludeScope=test", "dependency:build-classpath", "-Dmdep.outputFile=target/voyantclair-classpath.txt"));
    if (dependencies.exitCode != 0) return PreparedJavaArtifacts.failure(dependencies.output);
    try {
      Path output = module.resolve("target/voyantclair-classpath.txt");
      return new PreparedJavaArtifacts(
          directories(module.resolve("target/classes")),
          directories(module.resolve("target/test-classes")),
          Files.exists(output) ? paths(Files.readString(output), false) : List.of(), module, "");
    } catch (Exception error) {
      return PreparedJavaArtifacts.failure(error.toString());
    }
  }

  private static PreparedJavaArtifacts gradle(Path sourceRoot) {
    Path module = moduleRoot(sourceRoot);
    if (!Files.isRegularFile(module.resolve("build.gradle")) && !Files.isRegularFile(module.resolve("build.gradle.kts"))) {
      return PreparedJavaArtifacts.failure("No conventional Gradle module found for " + sourceRoot);
    }
    Path root = gradleRoot(module);
    Path wrapper = root.resolve("gradlew");
    String command = Files.isExecutable(wrapper) ? wrapper.toString() : System.getProperty("voyantclair.gradle.command", "gradle");
    try {
      Path init = Files.createTempFile("voyantclair-java-artifacts-", ".gradle");
      Files.writeString(init, initScript());
      try {
        List<String> configured = gradleArguments();
        ProcessResult discovery = run(command, root, invocation(configured, "-q", "-I", init.toString(), "voyantclairProjectPaths"));
        if (discovery.exitCode != 0) return PreparedJavaArtifacts.failure(discovery.output);
        String project = discoveredProjectPath(discovery.output, module).orElse(projectPath(root, module));
        ProcessResult compilation = run(command, root, invocation(configured, project + ":testClasses"));
        if (compilation.exitCode != 0) return PreparedJavaArtifacts.failure(compilation.output);
        ProcessResult artifacts = run(command, root, invocation(configured, "-q", "-I", init.toString(), project + ":voyantclairArtifacts"));
        if (artifacts.exitCode != 0) return PreparedJavaArtifacts.failure(artifacts.output);
        List<Path> main = markedPaths(artifacts.output, "VOYANTCLAIR_MAIN=", true);
        List<Path> test = markedPaths(artifacts.output, "VOYANTCLAIR_TEST=", true);
        if (main.isEmpty() && test.isEmpty()) return PreparedJavaArtifacts.failure("Gradle did not report Java class directories");
        return new PreparedJavaArtifacts(main, test, markedPaths(artifacts.output, "VOYANTCLAIR_TEST_RUNTIME=", false), module, "");
      } finally { Files.deleteIfExists(init); }
    } catch (Exception error) { return PreparedJavaArtifacts.failure(error.toString()); }
  }

  private static Path moduleRoot(Path sourceRoot) {
    Path absolute = sourceRoot.toAbsolutePath().normalize();
    Path main = absolute.getParent(); Path source = main == null ? null : main.getParent(); Path module = source == null ? null : source.getParent();
    return main != null && source != null && module != null && absolute.getFileName().toString().equals("java") && main.getFileName().toString().equals("main") && source.getFileName().toString().equals("src") ? module : absolute;
  }

  private static Path reactorPom(Path pom) {
    String name = pom.getParent().getFileName().toString();
    for (Path parent = pom.getParent().getParent(); parent != null; parent = parent.getParent()) {
      Path candidate = parent.resolve("pom.xml");
      try { if (Files.readString(candidate).contains("<module>" + name + "</module>")) return candidate; } catch (Exception ignored) { }
    }
    return pom;
  }

  private static ProcessResult run(String command, Path directory, List<String> arguments) {
    try {
      var invocation = new ArrayList<String>(); invocation.add(command); invocation.addAll(arguments);
      ProcessBuilder builder = new ProcessBuilder(invocation).directory(directory.toFile()).redirectErrorStream(true);
      String javaHome = System.getenv("JAVA_CALLGRAPH_MAVEN_JAVA_HOME");
      if (javaHome == null || javaHome.isBlank()) javaHome = System.getenv("WALA_MAVEN_JAVA_HOME");
      if (command.equals("mvn") && javaHome != null && !javaHome.isBlank()) builder.environment().put("JAVA_HOME", javaHome);
      Process process = builder.start(); String output = new String(process.getInputStream().readAllBytes()); return new ProcessResult(process.waitFor(), output);
    } catch (Exception error) { return new ProcessResult(-1, error.toString()); }
  }

  private static List<Path> directories(Path path) { return Files.isDirectory(path) ? List.of(path) : List.of(); }
  private static List<Path> paths(String value, boolean directories) { var result = new ArrayList<Path>(); for (String entry : value.split(java.util.regex.Pattern.quote(File.pathSeparator))) { if (entry.isBlank()) continue; Path path = Path.of(entry.trim()); if ((directories && Files.isDirectory(path)) || (!directories && Files.isRegularFile(path))) result.add(path); } return result; }
  private static List<Path> markedPaths(String output, String prefix, boolean directories) { var result = new ArrayList<Path>(); for (String line : output.lines().toList()) if (line.startsWith(prefix)) result.addAll(paths(line.substring(prefix.length()), directories)); return result; }
  private static List<String> gradleArguments() { String value = System.getProperty("voyantclair.gradle.arguments"); if (value == null || value.isBlank()) value = System.getenv("JAVA_CALLGRAPH_GRADLE_ARGUMENTS"); if (value == null || value.isBlank()) value = System.getenv("WALA_GRADLE_ARGUMENTS"); return value == null || value.isBlank() ? List.of() : List.of(value.trim().split("\\s+")); }
  private static List<String> invocation(List<String> configured, String... arguments) { var result = new ArrayList<>(configured); result.addAll(List.of(arguments)); return result; }
  private static Path gradleRoot(Path module) { for (Path current = module; current != null; current = current.getParent()) if (Files.isExecutable(current.resolve("gradlew")) || Files.isRegularFile(current.resolve("settings.gradle")) || Files.isRegularFile(current.resolve("settings.gradle.kts"))) return current; return module; }
  private static java.util.Optional<String> discoveredProjectPath(String output, Path module) { Path expected = module.toAbsolutePath().normalize(); for (String line : output.lines().toList()) if (line.startsWith("VOYANTCLAIR_PROJECT=")) { String[] fields = line.substring("VOYANTCLAIR_PROJECT=".length()).split("\\t", 2); if (fields.length == 2 && Path.of(fields[1]).toAbsolutePath().normalize().equals(expected)) return java.util.Optional.of(fields[0]); } return java.util.Optional.empty(); }
  private static String projectPath(Path root, Path module) { Path relative = root.relativize(module); return relative.getNameCount() == 0 ? "" : ":" + relative.toString().replace(File.separator, ":"); }
  private static String initScript() { return "gradle.projectsLoaded { rootProject.tasks.register('voyantclairProjectPaths') { doLast { rootProject.allprojects.each { project -> println 'VOYANTCLAIR_PROJECT=' + project.path + '\\t' + project.projectDir } } }; rootProject.allprojects { project -> project.tasks.register('voyantclairArtifacts') { doLast { def sets = project.extensions.findByName('sourceSets'); if (sets != null) { println 'VOYANTCLAIR_MAIN=' + sets.main.output.classesDirs.asPath; println 'VOYANTCLAIR_TEST=' + sets.test.output.classesDirs.asPath; println 'VOYANTCLAIR_TEST_RUNTIME=' + project.configurations.testRuntimeClasspath.asPath } } } } }"; }
  private record ProcessResult(int exitCode, String output) {}
}
