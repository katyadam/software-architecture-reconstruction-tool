package com.voyantclair.wala;

import com.ibm.wala.cast.java.client.impl.ZeroOneContainerCFABuilderFactory;
import com.ibm.wala.classLoader.BinaryDirectoryTreeModule;
import com.ibm.wala.ipa.callgraph.AnalysisCacheImpl;
import com.ibm.wala.ipa.callgraph.AnalysisOptions;
import com.ibm.wala.ipa.callgraph.AnalysisScope;
import com.ibm.wala.ipa.callgraph.CallGraph;
import com.ibm.wala.ipa.callgraph.Entrypoint;
import com.ibm.wala.ipa.callgraph.cha.CHACallGraph;
import com.ibm.wala.ipa.callgraph.impl.DefaultEntrypoint;
import com.ibm.wala.ipa.callgraph.impl.Util;
import com.ibm.wala.ipa.cha.ClassHierarchyFactory;
import com.ibm.wala.ipa.cha.IClassHierarchy;
import com.ibm.wala.ssa.SymbolTable;
import com.ibm.wala.types.ClassLoaderReference;
import com.ibm.wala.util.config.PatternsFilter;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.jar.JarFile;

/** Builds a WALA call graph from compiled application classes and dependency JARs. */
public final class BytecodeCallGraph {
  private static final String PROVIDER_ID = "wala-java";

  /** Selects the precision and expected computational cost of binary call-graph construction. */
  public enum Algorithm {
    /** Class Hierarchy Analysis, which is the conservative scalable default for service-sized scopes. */
    CHA("cha_bytecode"),
    /** Rapid Type Analysis, which trades some additional work for fewer conservative targets. */
    RTA("rta_bytecode"),
    /** Context-sensitive 0-1-container-CFA retained for callers that explicitly need precision. */
    ZERO_ONE_CONTAINER_CFA("zero_one_container_cfa_bytecode");

    private final String identifier;

    /** Creates an algorithm option with its stable JSON contract identifier. */
    Algorithm(String identifier) {
      this.identifier = identifier;
    }

    /** Returns the algorithm name published in provider results. */
    public String identifier() {
      return identifier;
    }
  }

  private BytecodeCallGraph() {}

  /**
   * Analyzes application class directories with scalable Class Hierarchy Analysis.
   *
   * @param applicationClassDirs compiled class directories that form the analyzed application
   * @param dependencyJars resolved third-party and sibling-module dependency JARs
   * @return a provider-contract result containing only edges called from application methods
   */
  public static CallGraphResult analyze(
      List<Path> applicationClassDirs, List<Path> dependencyJars) {
    return analyze(applicationClassDirs, dependencyJars, Algorithm.CHA);
  }

  /**
   * Analyzes application class directories using the requested WALA precision level.
   *
   * @param applicationClassDirs compiled class directories that form the analyzed application
   * @param dependencyJars resolved third-party and sibling-module dependency JARs
   * @param algorithm WALA strategy balancing scalable execution and call-target precision
   * @return a provider-contract result containing only edges called from application methods
   */
  public static CallGraphResult analyze(
      List<Path> applicationClassDirs, List<Path> dependencyJars, Algorithm algorithm) {
    long startedAtNanos = System.nanoTime();
    try {
      return withDuration(
          analyzeScope(applicationClassDirs, dependencyJars, algorithm), startedAtNanos);
    } catch (Throwable error) {
      return withDuration(failed(applicationClassDirs, algorithm, error), startedAtNanos);
    }
  }

  /** Adds elapsed binary-analysis time to the result without changing the shared JSON schema. */
  private static CallGraphResult withDuration(CallGraphResult result, long startedAtNanos) {
    var diagnostics = new ArrayList<>(result.diagnostics());
    diagnostics.add("analysis_duration_ms=" + (System.nanoTime() - startedAtNanos) / 1_000_000);
    return new CallGraphResult(
        result.schema_version(),
        result.status(),
        result.provider_id(),
        result.source_root(),
        result.algorithm(),
        diagnostics,
        result.edges());
  }

  /** Creates the binary WALA scope, discovers entry points, and extracts application-originating edges. */
  private static CallGraphResult analyzeScope(
      List<Path> applicationClassDirs, List<Path> dependencyJars, Algorithm algorithm) throws Exception {
    var scope = AnalysisScope.createJavaAnalysisScope();
    scope.setExclusions(
        PatternsFilter.builder()
            .addAll(
                BytecodeCallGraph.class
                    .getClassLoader()
                    .getResourceAsStream("Java60RegressionExclusions.txt"))
            .build());
    scope.addToScope(ClassLoaderReference.Primordial, new JarFile(javaRuntimeJar()));
    for (Path classDir : applicationClassDirs) {
      scope.addToScope(
          ClassLoaderReference.Application, new BinaryDirectoryTreeModule(classDir.toFile()));
    }
    for (Path dependencyJar : dependencyJars) {
      scope.addToScope(ClassLoaderReference.Extension, new JarFile(dependencyJar.toFile()));
    }

    IClassHierarchy hierarchy = ClassHierarchyFactory.make(scope);
    var entryPoints = applicationEntrypoints(hierarchy);
    if (entryPoints.isEmpty()) {
      return result("no_entrypoints", applicationClassDirs, algorithm, List.of());
    }

    var options = new AnalysisOptions(scope, entryPoints);
    options.getSSAOptions().setDefaultValues(SymbolTable::getDefaultValue);
    var callGraph = buildCallGraph(algorithm, options, hierarchy, scope);

    var edges = new ArrayList<CallGraphResult.Edge>();
    for (var caller : callGraph) {
      if (!scope.isApplicationLoader(caller.getMethod().getDeclaringClass().getClassLoader())) {
        continue;
      }
      var callees = callGraph.getSuccNodes(caller);
      while (callees.hasNext()) {
        edges.add(
            new CallGraphResult.Edge(
                ref(caller.getMethod()),
                ref(callees.next().getMethod()),
                PROVIDER_ID,
                algorithm.identifier(),
                1f));
      }
    }
    return result("ok", applicationClassDirs, algorithm, edges);
  }

  /** Builds a WALA call graph with the requested precision strategy. */
  private static CallGraph buildCallGraph(
      Algorithm algorithm, AnalysisOptions options, IClassHierarchy hierarchy, AnalysisScope scope)
      throws Exception {
    return switch (algorithm) {
      case CHA -> {
        var callGraph = new CHACallGraph(hierarchy);
        var entryPoints = new ArrayList<Entrypoint>();
        options.getEntrypoints().forEach(entryPoints::add);
        callGraph.init(entryPoints);
        yield callGraph;
      }
      case RTA ->
          Util.makeRTABuilder(options, new AnalysisCacheImpl(), hierarchy, scope)
              .makeCallGraph(options, null);
      case ZERO_ONE_CONTAINER_CFA ->
          new ZeroOneContainerCFABuilderFactory()
              .make(options, new AnalysisCacheImpl(), hierarchy)
              .makeCallGraph(options, null);
    };
  }

  /** Returns every application method that has the conventional Java {@code main} signature. */
  private static List<Entrypoint> applicationEntrypoints(IClassHierarchy hierarchy) {
    var entryPoints = new ArrayList<Entrypoint>();
    for (var clazz : hierarchy) {
      if (!clazz.getClassLoader().getReference().equals(ClassLoaderReference.Application)) {
        continue;
      }
      for (var method : clazz.getDeclaredMethods()) {
        if (method.isPublic()
            && method.isStatic()
            && method.getName().toString().equals("main")
            && method.getDescriptor().toString().equals("([Ljava/lang/String;)V")) {
          entryPoints.add(new DefaultEntrypoint(method, hierarchy));
        }
      }
    }
    return entryPoints;
  }

  /** Returns the Java runtime archive configured for WALA's primordial scope. */
  private static String javaRuntimeJar() {
    return System.getProperty(
        "voyantclair.wala.rtJar",
        System.getenv()
            .getOrDefault(
                "VOYANTCLAIR_WALA_RT_JAR",
                "/Library/Java/JavaVirtualMachines/temurin-8.jdk/Contents/Home/jre/lib/rt.jar"));
  }

  /** Converts a WALA method into the provider-neutral method identity used by VoyantClair. */
  private static CallGraphResult.MethodRef ref(com.ibm.wala.classLoader.IMethod method) {
    return new CallGraphResult.MethodRef(
        "java",
        method.getDeclaringClass().getName().toString(),
        method.getName().toString(),
        method.getDescriptor().toString(),
        null,
        null);
  }

  /** Builds a successful or no-entrypoint result with stable bytecode-provider metadata. */
  private static CallGraphResult result(
      String status,
      List<Path> applicationClassDirs,
      Algorithm algorithm,
      List<CallGraphResult.Edge> edges) {
    return new CallGraphResult(
        1,
        status,
        PROVIDER_ID,
        applicationClassDirs.toString(),
        algorithm.identifier(),
        List.of(),
        edges);
  }

  /** Converts binary scope or call-graph failures into a consumable provider result. */
  private static CallGraphResult failed(
      List<Path> applicationClassDirs, Algorithm algorithm, Throwable error) {
    return new CallGraphResult(
        1,
        "failed",
        PROVIDER_ID,
        applicationClassDirs.toString(),
        algorithm.identifier(),
        List.of(error.getClass().getSimpleName() + ": " + error.getMessage()),
        List.of());
  }
}
