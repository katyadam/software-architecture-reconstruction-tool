package com.voyantclair.wala;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.ibm.wala.cast.ir.ssa.AstIRFactory;
import com.ibm.wala.cast.java.client.impl.ZeroOneContainerCFABuilderFactory;
import com.ibm.wala.cast.java.ipa.callgraph.JavaSourceAnalysisScope;
import com.ibm.wala.cast.java.translator.jdt.ecj.ECJClassLoaderFactory;
import com.ibm.wala.classLoader.SourceDirectoryTreeModule;
import com.ibm.wala.ipa.callgraph.AnalysisCacheImpl;
import com.ibm.wala.ipa.callgraph.AnalysisOptions;
import com.ibm.wala.ipa.callgraph.Entrypoint;
import com.ibm.wala.ipa.callgraph.impl.DefaultEntrypoint;
import com.ibm.wala.ipa.cha.ClassHierarchyFactory;
import com.ibm.wala.ipa.cha.IClassHierarchy;
import com.ibm.wala.ssa.SymbolTable;
import com.ibm.wala.types.ClassLoaderReference;
import com.ibm.wala.util.config.PatternsFilter;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.jar.JarFile;

/** Runs WALA's source-based Java analysis and serializes its call graph for VoyantClair. */
public final class WalaCallGraphMain {
  private static final ObjectMapper JSON = new ObjectMapper();

  /**
   * Builds a 0-1-container-CFA call graph for every explicit Java {@code main} method below
   * {@code sourceDir}.
   *
   * @param sourceDir source-tree root supplied by VoyantClair
   * @return a versioned provider-neutral result, including an empty result when no entry point exists
   */
  public static CallGraphResult run(Path sourceDir) {
    return run(sourceDir, resolveMavenClasspath(sourceDir));
  }

  /**
   * Builds a call graph using explicitly supplied dependency JARs in addition to the Java runtime.
   *
   * @param sourceDir source-tree root supplied by VoyantClair
   * @param dependencyJars compile-time dependency JARs required to bind referenced external types
   * @return a versioned provider-neutral result, including an empty result when no entry point exists
   */
  public static CallGraphResult run(Path sourceDir, List<Path> dependencyJars) {
    try {
      return runAnalysis(sourceDir, dependencyJars);
    } catch (Throwable error) {
      return failed(sourceDir, error);
    }
  }

  /** Runs the binary WALA backend over already-compiled application class directories. */
  public static CallGraphResult runBytecode(
      List<Path> applicationClassDirs, List<Path> dependencyJars) {
    return BytecodeCallGraph.analyze(applicationClassDirs, dependencyJars);
  }

  /** Runs the binary WALA backend with an explicitly selected precision strategy. */
  public static CallGraphResult runBytecode(
      List<Path> applicationClassDirs,
      List<Path> dependencyJars,
      BytecodeCallGraph.Algorithm algorithm) {
    return BytecodeCallGraph.analyze(applicationClassDirs, dependencyJars, algorithm);
  }

  /** Runs binary WALA with explicit compiled methods as additional graph roots. */
  public static CallGraphResult runBytecode(
      List<Path> applicationClassDirs,
      List<Path> dependencyJars,
      List<BytecodeCallGraph.MethodSelector> entrypointSelectors,
      BytecodeCallGraph.Algorithm algorithm) {
    return BytecodeCallGraph.analyze(
        applicationClassDirs, dependencyJars, entrypointSelectors, algorithm);
  }

  /** Prefers Maven-prepared bytecode and falls back to source analysis when preparation is unavailable. */
  public static CallGraphResult runPreferred(Path sourceDir, List<Path> explicitClassDirs, List<Path> dependencyJars) {
    return runPreferred(
        sourceDir, explicitClassDirs, dependencyJars, BytecodeCallGraph.Algorithm.CHA);
  }

  /** Prefers Maven-prepared bytecode using a caller-selected strategy before source fallback. */
  public static CallGraphResult runPreferred(
      Path sourceDir,
      List<Path> explicitClassDirs,
      List<Path> dependencyJars,
      BytecodeCallGraph.Algorithm algorithm) {
    return runPreferred(sourceDir, explicitClassDirs, dependencyJars, List.of(), algorithm);
  }

  /** Prefers Maven bytecode and binds selected test methods as explicit WALA graph roots. */
  public static CallGraphResult runPreferred(
      Path sourceDir,
      List<Path> explicitClassDirs,
      List<Path> dependencyJars,
      List<BytecodeCallGraph.MethodSelector> entrypointSelectors,
      BytecodeCallGraph.Algorithm algorithm) {
    if (!explicitClassDirs.isEmpty()) {
      return runBytecode(explicitClassDirs, dependencyJars, entrypointSelectors, algorithm);
    }
    var preparation = MavenBytecodeArtifacts.prepare(sourceDir);
    if (preparation.succeeded()) {
      var artifacts = preparation.artifacts();
      var combinedDependencies = new ArrayList<Path>(artifacts.dependencyJars());
      combinedDependencies.addAll(dependencyJars);
      var combinedClassDirs = new ArrayList<Path>(artifacts.applicationClassDirs());
      combinedClassDirs.addAll(artifacts.testClassDirs());
      return runBytecode(combinedClassDirs, combinedDependencies, entrypointSelectors, algorithm);
    }
    if (!entrypointSelectors.isEmpty()) {
      return testRootPreparationFailed(sourceDir, algorithm, preparation.diagnostic());
    }
    return run(sourceDir, dependencyJars);
  }

  /** Reports Maven preparation failure instead of analyzing unrelated main-method paths for TIA. */
  private static CallGraphResult testRootPreparationFailed(
      Path sourceDir, BytecodeCallGraph.Algorithm algorithm, String diagnostic) {
    return new CallGraphResult(
        1,
        "failed",
        "wala-java",
        sourceDir.toString(),
        algorithm.identifier(),
        List.of("test_root_preparation_failed=" + diagnostic),
        List.of());
  }

  /** Converts the public CLI algorithm name to the corresponding binary WALA strategy. */
  static BytecodeCallGraph.Algorithm parseBytecodeAlgorithm(String value) {
    return switch (value) {
      case "cha" -> BytecodeCallGraph.Algorithm.CHA;
      case "rta" -> BytecodeCallGraph.Algorithm.RTA;
      case "zero-one-container-cfa" -> BytecodeCallGraph.Algorithm.ZERO_ONE_CONTAINER_CFA;
      default -> throw new IllegalArgumentException("Unsupported bytecode algorithm: " + value);
    };
  }

  /** Parses one stable {@code Lpackage/Class#method(descriptor)} command-line selector. */
  static BytecodeCallGraph.MethodSelector parseMethodSelector(String value) {
    int separator = value.indexOf('#');
    int descriptorStart = value.indexOf('(', separator + 1);
    int descriptorEnd = value.indexOf(')', descriptorStart + 1);
    if (separator <= 0
        || descriptorStart <= separator + 1
        || descriptorEnd < descriptorStart
        || descriptorEnd == value.length() - 1) {
      throw new IllegalArgumentException("Invalid --entrypoint selector: " + value);
    }
    return new BytecodeCallGraph.MethodSelector(
        value.substring(0, separator),
        value.substring(separator + 1, descriptorStart),
        value.substring(descriptorStart));
  }

  /** Performs the WALA analysis after callers have resolved any required dependency JARs. */
  private static CallGraphResult runAnalysis(Path sourceDir, List<Path> dependencyJars)
      throws Exception {
    try (var files = Files.walk(sourceDir)) {
      // Avoid WALA setup for libraries and service modules without an executable entry point.
      if (files.filter(path -> path.toString().endsWith(".java")).noneMatch(WalaCallGraphMain::hasMain)) {
        return result("no_entrypoints", sourceDir, List.of());
      }
    }

    var scope = new JavaSourceAnalysisScope();
    scope.setExclusions(
        PatternsFilter.builder()
            .addAll(
                WalaCallGraphMain.class
                    .getClassLoader()
                    .getResourceAsStream("Java60RegressionExclusions.txt"))
            .build());
    String runtimeJar =
        System.getProperty(
            "voyantclair.wala.rtJar",
            System.getenv()
                .getOrDefault(
                    "VOYANTCLAIR_WALA_RT_JAR",
                    "/Library/Java/JavaVirtualMachines/temurin-8.jdk/Contents/Home/jre/lib/rt.jar"));
    scope.addToScope(ClassLoaderReference.Primordial, new JarFile(runtimeJar));
    for (Path dependencyJar : dependencyJars) {
      scope.addToScope(ClassLoaderReference.Extension, new JarFile(dependencyJar.toFile()));
    }
    scope.addToScope(JavaSourceAnalysisScope.SOURCE, new SourceDirectoryTreeModule(sourceDir.toFile()));

    IClassHierarchy cha =
        ClassHierarchyFactory.make(scope, new ECJClassLoaderFactory(scope.getExclusions()));
    var entries = new ArrayList<Entrypoint>();
    for (var clazz : cha) {
      for (var method : clazz.getDeclaredMethods()) {
        if (method.isPublic()
            && method.isStatic()
            && method.getName().toString().equals("main")
            && method.getDescriptor().toString().equals("([Ljava/lang/String;)V")) {
          entries.add(new DefaultEntrypoint(method, cha));
        }
      }
    }
    if (entries.isEmpty()) {
      return result("no_entrypoints", sourceDir, List.of());
    }

    var options = new AnalysisOptions(scope, entries);
    options.getSSAOptions().setDefaultValues(SymbolTable::getDefaultValue);
    var cache = new AnalysisCacheImpl(AstIRFactory.makeDefaultFactory(), options.getSSAOptions());
    var callGraph =
        new ZeroOneContainerCFABuilderFactory().make(options, cache, cha).makeCallGraph(options, null);

    var edges = new ArrayList<CallGraphResult.Edge>();
    for (var node : callGraph) {
      var successors = callGraph.getSuccNodes(node);
      while (successors.hasNext()) {
        var successor = successors.next();
        edges.add(
            new CallGraphResult.Edge(
                ref(node.getMethod()),
                ref(successor.getMethod()),
                "wala-java",
                "zero_one_container_cfa",
                1f));
      }
    }
    return result("ok", sourceDir, edges);
  }

  /**
   * Locates the nearest Maven project and asks Maven for its compile dependency classpath.
   *
   * <p>Returns no JARs when the source tree is not inside a Maven project or Maven cannot resolve
   * dependencies, allowing callers that supply an explicit classpath to remain independent of Maven.
   */
  private static List<Path> resolveMavenClasspath(Path sourceDir) {
    Path pom = findMavenProject(sourceDir);
    if (pom == null) {
      return List.of();
    }

    try {
      Path classpathFile = Files.createTempFile("voyantclair-wala-classpath-", ".txt");
      try {
        var process =
            new ProcessBuilder(
                    "mvn",
                    "-q",
                    "-f",
                    pom.toString(),
                    "-DincludeScope=compile",
                    "dependency:build-classpath",
                    "-Dmdep.outputFile=" + classpathFile)
                .redirectErrorStream(true)
                .start();
        if (process.waitFor() != 0 || !Files.exists(classpathFile)) {
          return List.of();
        }
        return parseClasspath(Files.readString(classpathFile));
      } finally {
        Files.deleteIfExists(classpathFile);
      }
    } catch (Exception ignored) {
      return List.of();
    }
  }

  /**
   * Finds a Maven project only when the analyzed tree is its conventional main Java source root.
   *
   * <p>This avoids treating arbitrary directories nested inside another Maven project, such as
   * adapter test fixtures, as application source trees.
   */
  private static Path findMavenProject(Path sourceDir) {
    Path absoluteSourceRoot = sourceDir.toAbsolutePath().normalize();
    Path main = absoluteSourceRoot.getParent();
    Path source = main == null ? null : main.getParent();
    Path project = source == null ? null : source.getParent();
    if (main == null
        || source == null
        || project == null
        || !absoluteSourceRoot.getFileName().toString().equals("java")
        || !main.getFileName().toString().equals("main")
        || !source.getFileName().toString().equals("src")) {
      return null;
    }
    Path pom = project.resolve("pom.xml");
    return Files.isRegularFile(pom) ? pom : null;
  }

  /** Converts Maven's platform-separated classpath output into readable dependency JAR paths. */
  private static List<Path> parseClasspath(String classpath) {
    return classpath.lines()
        .flatMap(line -> List.of(line.split(java.util.regex.Pattern.quote(java.io.File.pathSeparator))).stream())
        .map(String::trim)
        .filter(entry -> !entry.isEmpty())
        .map(Path::of)
        .filter(Files::isRegularFile)
        .toList();
  }

  /** Returns whether a source file syntactically appears to contain a conventional Java entry point. */
  private static boolean hasMain(Path sourceFile) {
    try {
      var source = Files.readString(sourceFile);
      return source.contains("static void main") && source.contains("String[]");
    } catch (Exception ignored) {
      // Unreadable files cannot provide an entry point and are left to WALA's normal diagnostics.
      return false;
    }
  }

  /** Converts a WALA method identity into the JSON contract shared with VoyantClair. */
  private static CallGraphResult.MethodRef ref(com.ibm.wala.classLoader.IMethod method) {
    return new CallGraphResult.MethodRef(
        "java",
        method.getDeclaringClass().getName().toString(),
        method.getName().toString(),
        method.getDescriptor().toString(),
        null,
        null);
  }

  /** Creates a result with stable metadata for this WALA provider and algorithm. */
  private static CallGraphResult result(
      String status, Path root, List<CallGraphResult.Edge> edges) {
    return new CallGraphResult(
        1,
        status,
        "wala-java",
        root.toString(),
        "zero_one_container_cfa",
        List.of(),
        edges);
  }

  /** Converts an analysis exception into a JSON result that downstream providers can consume safely. */
  private static CallGraphResult failed(Path root, Throwable error) {
    return new CallGraphResult(
        1,
        "failed",
        "wala-java",
        root.toString(),
        "zero_one_container_cfa",
        List.of(error.getClass().getSimpleName() + ": " + error.getMessage()),
        List.of());
  }

  /** Parses CLI arguments, runs analysis, and writes exactly one JSON result to standard output. */
  public static void main(String[] args) throws Exception {
    Path sourceDir = null;
    var dependencyJars = new ArrayList<Path>();
    var applicationClassDirs = new ArrayList<Path>();
    var entrypointSelectors = new ArrayList<BytecodeCallGraph.MethodSelector>();
    var bytecodeAlgorithm = BytecodeCallGraph.Algorithm.CHA;
    for (int index = 0; index < args.length; index++) {
      if (args[index].equals("--source-dir") && index + 1 < args.length) {
        sourceDir = Path.of(args[++index]);
      } else if (args[index].equals("--classpath") && index + 1 < args.length) {
        dependencyJars.addAll(parseClasspath(args[++index]));
      } else if (args[index].equals("--classes-dir") && index + 1 < args.length) {
        applicationClassDirs.add(Path.of(args[++index]));
      } else if (args[index].equals("--test-classes-dir") && index + 1 < args.length) {
        applicationClassDirs.add(Path.of(args[++index]));
      } else if (args[index].equals("--entrypoint") && index + 1 < args.length) {
        entrypointSelectors.add(parseMethodSelector(args[++index]));
      } else if (args[index].equals("--algorithm") && index + 1 < args.length) {
        bytecodeAlgorithm = parseBytecodeAlgorithm(args[++index]);
      }
    }
    if (sourceDir == null) {
      throw new IllegalArgumentException("Missing required --source-dir argument");
    }
    System.out.println(
        JSON.writeValueAsString(
            runPreferred(
                sourceDir,
                applicationClassDirs,
                dependencyJars,
                entrypointSelectors,
                bytecodeAlgorithm)));
  }
}
