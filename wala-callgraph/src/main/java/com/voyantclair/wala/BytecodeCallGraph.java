package com.voyantclair.wala;

import com.ibm.wala.cast.java.client.impl.ZeroOneContainerCFABuilderFactory;
import com.ibm.wala.classLoader.BinaryDirectoryTreeModule;
import com.ibm.wala.ipa.callgraph.AnalysisCacheImpl;
import com.ibm.wala.ipa.callgraph.AnalysisOptions;
import com.ibm.wala.ipa.callgraph.AnalysisScope;
import com.ibm.wala.ipa.callgraph.CallGraph;
import com.ibm.wala.ipa.callgraph.CGNode;
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
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.BitSet;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.concurrent.atomic.AtomicBoolean;
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

  /** Identifies one compiled application or test method that must be a WALA entry point. */
  public record MethodSelector(String declaringType, String memberName, String descriptor) {}
  /** Identifies a changed source method without requiring Rust to reconstruct its JVM descriptor. */
  public record TargetSelector(String declaringType, String memberName, int parameterCount) {}

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
    return analyze(applicationClassDirs, dependencyJars, List.of(), List.of(), algorithm);
  }

  /** Analyzes bytecode with explicit method roots in addition to conventional Java main methods. */
  public static CallGraphResult analyze(
      List<Path> applicationClassDirs,
      List<Path> dependencyJars,
      List<MethodSelector> entrypointSelectors,
      Algorithm algorithm) {
    return analyze(applicationClassDirs, dependencyJars, entrypointSelectors, List.of(), algorithm);
  }

  /** Analyzes bytecode while computing provenance only for explicitly changed source targets. */
  public static CallGraphResult analyze(
      List<Path> applicationClassDirs,
      List<Path> dependencyJars,
      List<MethodSelector> entrypointSelectors,
      List<TargetSelector> targetSelectors,
      Algorithm algorithm) {
    long startedAtNanos = System.nanoTime();
    try {
      return withDuration(
          analyzeScope(applicationClassDirs, dependencyJars, entrypointSelectors, targetSelectors, algorithm), startedAtNanos);
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
        result.reachability(),
        result.edges());
  }

  /** Creates the binary WALA scope, discovers entry points, and extracts application-originating edges. */
  private static CallGraphResult analyzeScope(
      List<Path> applicationClassDirs,
      List<Path> dependencyJars,
      List<MethodSelector> entrypointSelectors,
      List<TargetSelector> targetSelectors,
      Algorithm algorithm)
      throws Exception {
    long startedAtNanos = System.nanoTime();
    phase("scope_start", startedAtNanos);
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
    phase("scope_ready", startedAtNanos);

    IClassHierarchy hierarchy = ClassHierarchyFactory.make(scope);
    phase("class_hierarchy_ready", startedAtNanos);
    var entryPoints = applicationEntrypoints(hierarchy);
    var selectedEntrypoints = selectedEntrypoints(hierarchy, entrypointSelectors);
    entryPoints.addAll(selectedEntrypoints.entryPoints());
    if (entryPoints.isEmpty()) {
      return result(
          "no_entrypoints",
          applicationClassDirs,
          algorithm,
          selectedEntrypoints.diagnostics(),
          List.of(),
          List.of());
    }

    var options = new AnalysisOptions(scope, entryPoints);
    options.getSSAOptions().setDefaultValues(SymbolTable::getDefaultValue);
    var callGraphBuilding = new AtomicBoolean(true);
    Thread watchdog = phaseWatchdog("call_graph_building", startedAtNanos, callGraphBuilding);
    CallGraph callGraph;
    try {
      callGraph = buildCallGraph(algorithm, options, hierarchy, scope);
    } finally {
      callGraphBuilding.set(false);
      watchdog.interrupt();
    }
    phase("call_graph_ready", startedAtNanos);

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
    phase("edges_ready", startedAtNanos);
    long reachabilityStartedAtNanos = System.nanoTime();
    var reachability = reachability(callGraph, scope, entrypointSelectors, targetSelectors);
    phase("reachability_ready", startedAtNanos);
    var diagnostics = new ArrayList<>(selectedEntrypoints.diagnostics());
    diagnostics.add(
        "reachability_duration_ms="
            + (System.nanoTime() - reachabilityStartedAtNanos) / 1_000_000);
    return result(
        "ok", applicationClassDirs, algorithm, diagnostics, reachability, edges);
  }

  /** Emits one bounded progress milestone to stderr without corrupting JSON on stdout. */
  private static void phase(String phase, long startedAtNanos) {
    System.err.println(
        "wala_phase="
            + phase
            + " elapsed_ms="
            + (System.nanoTime() - startedAtNanos) / 1_000_000);
    System.err.flush();
  }

  /** Reports a stalled WALA phase without interrupting the analysis thread or changing its result. */
  private static Thread phaseWatchdog(
      String phase, long startedAtNanos, AtomicBoolean active) {
    Thread analysisThread = Thread.currentThread();
    Thread watchdog = new Thread(
        () -> {
          while (active.get()) {
            try {
              Thread.sleep(15_000);
            } catch (InterruptedException ignored) {
              return;
            }
            if (!active.get()) {
              return;
            }
            StackTraceElement[] stack = analysisThread.getStackTrace();
            String topFrame = stack.length == 0 ? "unavailable" : stack[0].toString();
            System.err.println(
                "wala_phase="
                    + phase
                    + " elapsed_ms="
                    + (System.nanoTime() - startedAtNanos) / 1_000_000
                    + " top_frame="
                    + topFrame);
            System.err.flush();
          }
        },
        "wala-phase-watchdog");
    watchdog.setDaemon(true);
    watchdog.start();
    return watchdog;
  }

  /** Computes test-root provenance through all WALA nodes while publishing application methods only. */
  private static List<CallGraphResult.Reachability> reachability(
      CallGraph callGraph, AnalysisScope scope, List<MethodSelector> selectors,
      List<TargetSelector> targetSelectors) {
    if (!targetSelectors.isEmpty()) {
      return targetReachability(callGraph, scope, selectors, targetSelectors);
    }
    var rootIndexes = new LinkedHashMap<MethodSelector, Integer>();
    var roots = new ArrayList<CallGraphResult.MethodRef>();
    for (MethodSelector selector : selectors) {
      if (!rootIndexes.containsKey(selector)) {
        rootIndexes.put(selector, roots.size());
        roots.add(new CallGraphResult.MethodRef(
            "java", selector.declaringType(), selector.memberName(), selector.descriptor(), null, null));
      }
    }

    var rootsByNode = new LinkedHashMap<CGNode, BitSet>();
    var pending = new ArrayDeque<CGNode>();
    for (CGNode node : callGraph) {
      Integer rootIndex = rootIndexes.get(selector(node));
      if (rootIndex == null) {
        continue;
      }
      var reachedRoots = rootsByNode.computeIfAbsent(node, ignored -> new BitSet(roots.size()));
      if (!reachedRoots.get(rootIndex)) {
        reachedRoots.set(rootIndex);
        pending.add(node);
      }
    }

    while (!pending.isEmpty()) {
      CGNode node = pending.removeFirst();
      BitSet nodeRoots = rootsByNode.get(node);
      var successors = callGraph.getSuccNodes(node);
      while (successors.hasNext()) {
        CGNode successor = successors.next();
        BitSet successorRoots =
            rootsByNode.computeIfAbsent(successor, ignored -> new BitSet(roots.size()));
        if (addRoots(successorRoots, nodeRoots)) {
          pending.add(successor);
        }
      }
    }

    var reachedByMethod = new LinkedHashMap<CallGraphResult.MethodRef, BitSet>();
    for (var entry : rootsByNode.entrySet()) {
      CGNode node = entry.getKey();
      if (!scope.isApplicationLoader(node.getMethod().getDeclaringClass().getClassLoader())) {
        continue;
      }
      BitSet methodRoots =
          reachedByMethod.computeIfAbsent(ref(node.getMethod()), ignored -> new BitSet(roots.size()));
      addRoots(methodRoots, entry.getValue());
    }
    return reachedByMethod.entrySet().stream()
        .map(entry -> new CallGraphResult.Reachability(entry.getKey(), roots(entry.getValue(), roots)))
        .toList();
  }

  /** Traverses predecessors from changed methods, avoiding full-graph provenance for every method. */
  private static List<CallGraphResult.Reachability> targetReachability(
      CallGraph callGraph, AnalysisScope scope, List<MethodSelector> roots, List<TargetSelector> targets) {
    var rootIndexes = new LinkedHashMap<MethodSelector, Integer>();
    var rootRefs = new ArrayList<CallGraphResult.MethodRef>();
    for (MethodSelector root : roots) {
      if (!rootIndexes.containsKey(root)) {
        rootIndexes.put(root, rootRefs.size());
        rootRefs.add(new CallGraphResult.MethodRef("java", root.declaringType(), root.memberName(), root.descriptor(), null, null));
      }
    }
    var rootNodes = new LinkedHashMap<CGNode, Integer>();
    for (CGNode entrypoint : callGraph.getEntrypointNodes()) {
      Integer root = rootIndexes.get(selector(entrypoint));
      if (root != null) rootNodes.put(entrypoint, root);
    }
    var reachedByMethod = new LinkedHashMap<CallGraphResult.MethodRef, BitSet>();
    for (CGNode target : callGraph) {
      if (!scope.isApplicationLoader(target.getMethod().getDeclaringClass().getClassLoader())
          || !targets.stream().anyMatch(selector -> matches(target, selector))) {
        continue;
      }
      var reachedRoots = reachedByMethod.computeIfAbsent(ref(target.getMethod()), ignored -> new BitSet(rootRefs.size()));
      var pending = new ArrayDeque<CGNode>();
      var visited = new HashSet<CGNode>();
      pending.add(target);
      while (!pending.isEmpty()) {
        CGNode node = pending.removeFirst();
        if (!visited.add(node)) continue;
        Integer root = rootNodes.get(node);
        if (root != null) reachedRoots.set(root);
        var predecessors = callGraph.getPredNodes(node);
        while (predecessors.hasNext()) pending.add(predecessors.next());
      }
    }
    return reachedByMethod.entrySet().stream()
        .map(entry -> new CallGraphResult.Reachability(entry.getKey(), roots(entry.getValue(), rootRefs)))
        .toList();
  }

  /** Adds source root bits to a destination and returns whether it acquired any new provenance. */
  private static boolean addRoots(BitSet destination, BitSet source) {
    boolean changed = false;
    for (int root = source.nextSetBit(0); root >= 0; root = source.nextSetBit(root + 1)) {
      if (!destination.get(root)) {
        destination.set(root);
        changed = true;
      }
    }
    return changed;
  }

  /** Materializes stable entrypoint references for the compact internal provenance bit set. */
  private static List<CallGraphResult.MethodRef> roots(
      BitSet indexes, List<CallGraphResult.MethodRef> roots) {
    var reachedRoots = new ArrayList<CallGraphResult.MethodRef>();
    for (int root = indexes.nextSetBit(0); root >= 0; root = indexes.nextSetBit(root + 1)) {
      reachedRoots.add(roots.get(root));
    }
    return List.copyOf(reachedRoots);
  }

  private static MethodSelector selector(CGNode node) {
    var method = node.getMethod();
    return new MethodSelector(
        method.getDeclaringClass().getName().toString(),
        method.getName().toString(),
        method.getDescriptor().toString());
  }

  private static boolean matches(CGNode node, TargetSelector selector) {
    var method = node.getMethod();
    return method.getDeclaringClass().getName().toString().equals(selector.declaringType())
        && method.getName().toString().equals(selector.memberName())
        && parameterCount(method.getDescriptor().toString()) == selector.parameterCount();
  }

  private static int parameterCount(String descriptor) {
    int count = 0;
    for (int index = 1; descriptor.charAt(index) != ')'; count++) {
      while (descriptor.charAt(index) == '[') index++;
      if (descriptor.charAt(index) == 'L') index = descriptor.indexOf(';', index) + 1;
      else index++;
    }
    return count;
  }

  /** Resolves explicit compiled method selectors only when they refer to application classes. */
  private static SelectedEntrypoints selectedEntrypoints(
      IClassHierarchy hierarchy, List<MethodSelector> selectors) {
    var entryPoints = new ArrayList<Entrypoint>();
    Set<MethodSelector> matchedSelectors = new HashSet<>();
    for (var clazz : hierarchy) {
      if (!clazz.getClassLoader().getReference().equals(ClassLoaderReference.Application)) {
        continue;
      }
      for (var selector : selectors) {
        if (!clazz.getName().toString().equals(selector.declaringType())) {
          continue;
        }
        for (var method : clazz.getDeclaredMethods()) {
          if (method.getName().toString().equals(selector.memberName())
              && method.getDescriptor().toString().equals(selector.descriptor())) {
            entryPoints.add(new DefaultEntrypoint(method, hierarchy));
            matchedSelectors.add(selector);
          }
        }
      }
    }
    var diagnostics = new ArrayList<String>();
    for (var selector : selectors) {
      if (!matchedSelectors.contains(selector)) {
        diagnostics.add("unmatched_entrypoint=" + selector.declaringType() + "#"
            + selector.memberName() + selector.descriptor());
      }
    }
    return new SelectedEntrypoints(entryPoints, diagnostics);
  }

  /** Couples resolved explicit entry points with diagnostics for selectors that could not bind. */
  private record SelectedEntrypoints(List<Entrypoint> entryPoints, List<String> diagnostics) {}

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
    return result(status, applicationClassDirs, algorithm, List.of(), List.of(), edges);
  }

  /** Builds a result while preserving warnings that must cause conservative TIA fallback. */
  private static CallGraphResult result(
      String status,
      List<Path> applicationClassDirs,
      Algorithm algorithm,
      List<String> diagnostics,
      List<CallGraphResult.Reachability> reachability,
      List<CallGraphResult.Edge> edges) {
    return new CallGraphResult(
        2,
        status,
        PROVIDER_ID,
        applicationClassDirs.toString(),
        algorithm.identifier(),
        diagnostics,
        reachability,
        edges);
  }

  /** Converts binary scope or call-graph failures into a consumable provider result. */
  private static CallGraphResult failed(
      List<Path> applicationClassDirs, Algorithm algorithm, Throwable error) {
    return new CallGraphResult(
        2,
        "failed",
        PROVIDER_ID,
        applicationClassDirs.toString(),
        algorithm.identifier(),
        List.of(error.getClass().getSimpleName() + ": " + error.getMessage()),
        List.of(),
        List.of());
  }
}
