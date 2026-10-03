package com.voyantclair.wala;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.stream.Stream;

/** Prepares compiled class directories and dependency JARs for a conventional Maven service. */
public final class MavenBytecodeArtifacts implements BytecodeArtifactPreparer {
  /** Creates a Maven artifact preparer for conventional Java source roots. */
  public MavenBytecodeArtifacts() {}

  /** Builds the selected module with upstream reactor modules and collects WALA bytecode inputs. */
  @Override
  public BytecodeArtifacts.Preparation prepare(Path sourceRoot) {
    Path modulePom = findModulePom(sourceRoot);
    if (modulePom == null) {
      return failure("No conventional Maven module POM found for " + sourceRoot);
    }
    Path moduleRoot = modulePom.getParent();
    Path reactorPom = findReactorPom(modulePom);
    String selector = reactorPom.getParent().relativize(moduleRoot).toString();
    var build = runMaven(reactorPom, List.of("-pl", selector, "-am", "install", "-DskipTests"));
    if (build.exitCode() != 0) {
      return failure(build.output());
    }
    var classpath =
        runMaven(
            modulePom,
            List.of("-DincludeScope=test", "dependency:build-classpath", "-Dmdep.outputFile=target/wala-classpath.txt"));
    if (classpath.exitCode() != 0) {
      return failure(classpath.output());
    }
    try {
      Path classpathFile = moduleRoot.resolve("target/wala-classpath.txt");
      List<Path> dependencyJars =
          Files.exists(classpathFile) ? parseClasspath(Files.readString(classpathFile)) : List.of();
      return new BytecodeArtifacts.Preparation(
          new BytecodeArtifacts.PreparedArtifacts(
              findClassDirs(reactorPom.getParent()),
              findTestClassDirs(reactorPom.getParent()),
              dependencyJars,
              moduleRoot),
          "");
    } catch (IOException error) {
      return failure(error.toString());
    }
  }

  /** Finds the module POM only for the conventional Maven {@code src/main/java} layout. */
  private static Path findModulePom(Path sourceRoot) {
    Path absolute = sourceRoot.toAbsolutePath().normalize();
    Path main = absolute.getParent();
    Path source = main == null ? null : main.getParent();
    Path module = source == null ? null : source.getParent();
    if (main == null || source == null || module == null || !absolute.getFileName().toString().equals("java") || !main.getFileName().toString().equals("main") || !source.getFileName().toString().equals("src")) {
      return null;
    }
    Path pom = module.resolve("pom.xml");
    return Files.isRegularFile(pom) ? pom : null;
  }

  /** Finds the nearest ancestor POM declaring the selected module as a direct reactor child. */
  private static Path findReactorPom(Path modulePom) {
    String moduleName = modulePom.getParent().getFileName().toString();
    for (Path parent = modulePom.getParent().getParent(); parent != null; parent = parent.getParent()) {
      Path candidate = parent.resolve("pom.xml");
      try {
        if (Files.readString(candidate).contains("<module>" + moduleName + "</module>")) {
          return candidate;
        }
      } catch (IOException ignored) {
        // Continue walking until the module's enclosing reactor is found.
      }
    }
    return modulePom;
  }

  /** Runs Maven for a POM and captures output for structured failure diagnostics. */
  private static ProcessResult runMaven(Path pom, List<String> arguments) {
    try {
      var command = new java.util.ArrayList<String>();
      command.add("mvn");
      command.add("-q");
      command.add("-f");
      command.add(pom.toString());
      command.addAll(arguments);
      var process = new ProcessBuilder(command).redirectErrorStream(true).start();
      String output = new String(process.getInputStream().readAllBytes());
      return new ProcessResult(process.waitFor(), output);
    } catch (Exception error) {
      return new ProcessResult(-1, error.toString());
    }
  }

  /** Locates compiled class directories produced below a direct-child Maven reactor. */
  private static List<Path> findClassDirs(Path reactorRoot) throws IOException {
    try (Stream<Path> paths = Files.walk(reactorRoot, 4)) {
      return paths.filter(path -> path.endsWith("target/classes") && Files.isDirectory(path)).toList();
    }
  }

  /** Locates test class directories produced below a direct-child Maven reactor. */
  private static List<Path> findTestClassDirs(Path reactorRoot) throws IOException {
    try (Stream<Path> paths = Files.walk(reactorRoot, 4)) {
      return paths
          .filter(path -> path.endsWith("target/test-classes") && Files.isDirectory(path))
          .toList();
    }
  }

  /** Converts Maven's platform-separated dependency classpath into readable JAR paths. */
  private static List<Path> parseClasspath(String classpath) {
    return classpath.lines()
        .flatMap(line -> List.of(line.split(java.util.regex.Pattern.quote(java.io.File.pathSeparator))).stream())
        .map(Path::of)
        .filter(Files::isRegularFile)
        .toList();
  }

  /** Creates a failed preparation result without exposing process failures as exceptions. */
  private static BytecodeArtifacts.Preparation failure(String diagnostic) {
    return new BytecodeArtifacts.Preparation(
        null, diagnostic.isBlank() ? "Maven preparation failed" : diagnostic);
  }

  /** Represents one completed Maven process invocation. */
  private record ProcessResult(int exitCode, String output) {}
}
