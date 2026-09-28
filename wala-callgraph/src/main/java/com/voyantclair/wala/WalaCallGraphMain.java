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
   * @throws Exception when WALA cannot construct its analysis scope or call graph
   */
  public static CallGraphResult run(Path sourceDir) throws Exception {
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

  /** Parses the CLI arguments, runs analysis, and writes exactly one JSON result to standard output. */
  public static void main(String[] args) throws Exception {
    Path root = Path.of(args[1]);
    System.out.println(JSON.writeValueAsString(run(root)));
  }
}
