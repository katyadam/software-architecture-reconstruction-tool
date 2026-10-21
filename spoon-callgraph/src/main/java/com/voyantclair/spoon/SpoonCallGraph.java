package com.voyantclair.spoon;

import com.voyantclair.callgraph.CallGraphResult;
import spoon.Launcher;
import spoon.reflect.code.CtInvocation;
import spoon.reflect.declaration.CtMethod;
import spoon.reflect.declaration.CtType;
import spoon.reflect.reference.CtExecutableReference;
import spoon.reflect.reference.CtTypeReference;
import spoon.reflect.visitor.filter.TypeFilter;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.ArrayDeque;
import java.util.BitSet;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;

/** Builds a source-level call graph from Java sources using Spoon AST analysis. */
public final class SpoonCallGraph {
  static final String PROVIDER_ID = "spoon-java";
  static final String ALGORITHM = "cha_source";

  private SpoonCallGraph() {}

  /** Identifies a method without requiring a compiled descriptor. */
  public record MethodSelector(String declaringType, String memberName, String descriptor) {}

  /** Identifies a changed source method by type, name and parameter count. */
  public record TargetSelector(String declaringType, String memberName, int parameterCount) {}

  /**
   * Analyzes Java source trees and returns a call graph result in schema-v2 format.
   *
   * <p>Works in noclasspath mode — compiled classes are not required. Entrypoints and targets
   * narrow the reachability computation without affecting edge collection.
   */
  public static CallGraphResult analyze(
      List<Path> sourceDirs,
      List<Path> classpath,
      List<MethodSelector> entrypointSelectors,
      List<TargetSelector> targetSelectors) {
    long startedAtNanos = System.nanoTime();
    try {
      return withDuration(analyzeInternal(sourceDirs, classpath, entrypointSelectors, targetSelectors), startedAtNanos);
    } catch (Throwable error) {
      return withDuration(failed(sourceDirs, error), startedAtNanos);
    }
  }

  private static CallGraphResult analyzeInternal(
      List<Path> sourceDirs,
      List<Path> classpath,
      List<MethodSelector> entrypointSelectors,
      List<TargetSelector> targetSelectors) {
    phase("launcher_start");

    var launcher = new Launcher();
    launcher.getEnvironment().setNoClasspath(true);
    launcher.getEnvironment().setComplianceLevel(17);
    launcher.getEnvironment().setAutoImports(false);
    launcher.getEnvironment().setCommentEnabled(false);
    for (Path sourceDir : sourceDirs) {
      launcher.addInputResource(sourceDir.toString());
    }
    if (!classpath.isEmpty()) {
      String[] cp = classpath.stream().map(Path::toString).toArray(String[]::new);
      launcher.getEnvironment().setSourceClasspath(cp);
    }

    var model = launcher.buildModel();
    phase("model_ready");

    // Collect all source types for type-hierarchy dispatch resolution.
    var allTypes = new LinkedHashMap<String, CtType<?>>();
    for (CtType<?> type : model.getAllTypes()) {
      allTypes.put(binaryName(type.getQualifiedName()), type);
    }

    // Collect all methods indexed by their selector for entrypoint binding.
    var methodsBySelector = new LinkedHashMap<MethodSelector, CallGraphResult.MethodRef>();
    for (CtType<?> type : model.getAllTypes()) {
      for (CtMethod<?> method : type.getMethods()) {
        methodsBySelector.put(selectorOf(type, method), refOf(type, method));
      }
    }

    // Build raw call edges by visiting every CtInvocation in all source types.
    var edges = new ArrayList<CallGraphResult.Edge>();
    var callerToCallees = new LinkedHashMap<String, Set<String>>();

    for (CtType<?> type : model.getAllTypes()) {
      for (CtMethod<?> callerMethod : type.getMethods()) {
        var callerRef = refOf(type, callerMethod);
        String callerKey = methodKey(callerRef);

        var invocations = callerMethod.getElements(new TypeFilter<>(CtInvocation.class));
        for (CtInvocation<?> invocation : invocations) {
          CtExecutableReference<?> exec = invocation.getExecutable();
          if (exec == null || exec.getDeclaringType() == null) {
            continue;
          }

          var calleeRef = refOf(exec);
          edges.add(new CallGraphResult.Edge(callerRef, calleeRef, PROVIDER_ID, ALGORITHM, 1f));
          callerToCallees
              .computeIfAbsent(callerKey, ignored -> new LinkedHashSet<>())
              .add(methodKey(calleeRef));
        }
      }
    }
    phase("edges_ready");

    // Compute reachability: for each target, BFS backwards through the inverted call graph
    // to find which entrypoints can reach it.
    var reachability = computeReachability(
        callerToCallees, methodsBySelector, entrypointSelectors, targetSelectors);
    phase("reachability_ready");

    return result("ok", sourceDirs, List.of(), reachability, edges);
  }

  /** Inverts the caller→callees map and BFS-propagates test root provenance. */
  private static List<CallGraphResult.Reachability> computeReachability(
      LinkedHashMap<String, Set<String>> callerToCallees,
      LinkedHashMap<MethodSelector, CallGraphResult.MethodRef> methodsBySelector,
      List<MethodSelector> entrypointSelectors,
      List<TargetSelector> targetSelectors) {
    if (entrypointSelectors.isEmpty()) {
      return List.of();
    }

    // Build inverted graph: callee → set of callers
    var calleeToCallers = new LinkedHashMap<String, Set<String>>();
    for (var entry : callerToCallees.entrySet()) {
      String caller = entry.getKey();
      for (String callee : entry.getValue()) {
        calleeToCallers.computeIfAbsent(callee, ignored -> new LinkedHashSet<>()).add(caller);
      }
    }

    // Index entrypoints by their key for fast BFS termination.
    var rootKeyToRef = new LinkedHashMap<String, CallGraphResult.MethodRef>();
    for (MethodSelector selector : entrypointSelectors) {
      CallGraphResult.MethodRef ref = methodsBySelector.get(selector);
      if (ref != null) {
        rootKeyToRef.put(methodKey(ref), ref);
      } else {
        // Entrypoint not found in source — record by selector directly.
        var syntheticRef = new CallGraphResult.MethodRef(
            "java", selector.declaringType(), selector.memberName(), selector.descriptor(), null, null);
        rootKeyToRef.put(methodKey(syntheticRef), syntheticRef);
      }
    }

    // For each target, walk predecessors to find reachable entrypoints.
    var result = new ArrayList<CallGraphResult.Reachability>();

    for (var entry : methodsBySelector.entrySet()) {
      MethodSelector selector = entry.getKey();
      CallGraphResult.MethodRef methodRef = entry.getValue();

      boolean isTarget = targetSelectors.isEmpty() || targetSelectors.stream().anyMatch(t -> matchesTarget(selector, t));
      if (!isTarget) {
        continue;
      }

      var reachableRoots = new ArrayList<CallGraphResult.MethodRef>();
      var visited = new LinkedHashSet<String>();
      var pending = new ArrayDeque<String>();
      pending.add(methodKey(methodRef));

      while (!pending.isEmpty()) {
        String current = pending.removeFirst();
        if (!visited.add(current)) {
          continue;
        }
        CallGraphResult.MethodRef rootRef = rootKeyToRef.get(current);
        if (rootRef != null) {
          reachableRoots.add(rootRef);
        }
        Set<String> callers = calleeToCallers.get(current);
        if (callers != null) {
          pending.addAll(callers);
        }
      }

      if (!reachableRoots.isEmpty()) {
        result.add(new CallGraphResult.Reachability(methodRef, List.copyOf(reachableRoots)));
      }
    }

    return List.copyOf(result);
  }

  private static boolean matchesTarget(MethodSelector selector, TargetSelector target) {
    return selector.declaringType().equals(target.declaringType())
        && selector.memberName().equals(target.memberName())
        && parameterCountFromDescriptor(selector.descriptor()) == target.parameterCount();
  }

  /** Converts a Spoon qualified name to the JVM binary name used in MethodRef. */
  static String binaryName(String qualifiedName) {
    if (qualifiedName == null) return "";
    // Convert com.example.Outer.Inner → Lcom/example/Outer$Inner
    // Simple heuristic: last dot-separated segments starting with uppercase are nested types.
    String[] parts = qualifiedName.split("\\.");
    var packageParts = new ArrayList<String>();
    var typeParts = new ArrayList<String>();
    boolean inType = false;
    for (String part : parts) {
      if (!inType && Character.isUpperCase(part.charAt(0))) {
        inType = true;
      }
      if (inType) {
        typeParts.add(part);
      } else {
        packageParts.add(part);
      }
    }
    String pkg = String.join("/", packageParts);
    String type = String.join("$", typeParts);
    return "L" + (pkg.isEmpty() ? "" : pkg + "/") + type;
  }

  /** Builds a MethodRef from a Spoon CtType + CtMethod (source declaration). */
  static CallGraphResult.MethodRef refOf(CtType<?> type, CtMethod<?> method) {
    return new CallGraphResult.MethodRef(
        "java",
        binaryName(type.getQualifiedName()),
        method.getSimpleName(),
        descriptorOf(method),
        null,
        method.getPosition().isValidPosition() ? method.getPosition().getLine() : null);
  }

  /** Builds a MethodRef from a Spoon CtExecutableReference (call-site reference). */
  static CallGraphResult.MethodRef refOf(CtExecutableReference<?> exec) {
    CtTypeReference<?> declaring = exec.getDeclaringType();
    String typeName = declaring != null ? binaryName(declaring.getQualifiedName()) : "Lunknown";
    return new CallGraphResult.MethodRef(
        "java",
        typeName,
        exec.getSimpleName(),
        descriptorOf(exec),
        null,
        null);
  }

  /** Produces a JVM-style method descriptor from a Spoon method declaration. */
  static String descriptorOf(CtMethod<?> method) {
    String params = method.getParameters().stream()
        .map(p -> typeDescriptor(p.getType()))
        .collect(Collectors.joining());
    String ret = typeDescriptor(method.getType());
    return "(" + params + ")" + ret;
  }

  /** Produces a JVM-style method descriptor from a Spoon executable reference. */
  static String descriptorOf(CtExecutableReference<?> exec) {
    String params = exec.getParameters().stream()
        .map(SpoonCallGraph::typeDescriptor)
        .collect(Collectors.joining());
    CtTypeReference<?> returnType = exec.getType();
    String ret = returnType != null ? typeDescriptor(returnType) : "V";
    return "(" + params + ")" + ret;
  }

  /** Maps a Spoon type reference to its single JVM type descriptor character or Lpackage/Class; form. */
  static String typeDescriptor(CtTypeReference<?> typeRef) {
    if (typeRef == null) return "V";
    // Handle arrays recursively.
    if (typeRef.isArray()) {
      // CtTypeReference.isArray() is true only for CtArrayTypeReference instances.
      return "[" + typeDescriptor(((spoon.reflect.reference.CtArrayTypeReference<?>) typeRef).getComponentType());
    }
    String qualified = typeRef.getQualifiedName();
    return switch (qualified) {
      case "void"    -> "V";
      case "boolean" -> "Z";
      case "byte"    -> "B";
      case "char"    -> "C";
      case "short"   -> "S";
      case "int"     -> "I";
      case "long"    -> "J";
      case "float"   -> "F";
      case "double"  -> "D";
      default -> {
        // Object type — convert qualified name to JVM binary format.
        String binaryNoL = binaryName(qualified).substring(1); // strip leading L
        yield "L" + binaryNoL + ";";
      }
    };
  }

  private static MethodSelector selectorOf(CtType<?> type, CtMethod<?> method) {
    return new MethodSelector(
        binaryName(type.getQualifiedName()),
        method.getSimpleName(),
        descriptorOf(method));
  }

  private static String methodKey(CallGraphResult.MethodRef ref) {
    return ref.declaring_type() + "#" + ref.member_name() + ref.descriptor();
  }

  private static int parameterCountFromDescriptor(String descriptor) {
    if (descriptor == null || descriptor.isEmpty()) return 0;
    int count = 0;
    int i = 1; // skip opening '('
    while (i < descriptor.length() && descriptor.charAt(i) != ')') {
      while (descriptor.charAt(i) == '[') i++;
      if (descriptor.charAt(i) == 'L') {
        i = descriptor.indexOf(';', i) + 1;
      } else {
        i++;
      }
      count++;
    }
    return count;
  }

  private static void phase(String phase) {
    System.err.println("spoon_phase=" + phase + " ts_ms=" + System.currentTimeMillis());
    System.err.flush();
  }

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

  private static CallGraphResult result(
      String status,
      List<Path> sourceDirs,
      List<String> diagnostics,
      List<CallGraphResult.Reachability> reachability,
      List<CallGraphResult.Edge> edges) {
    return new CallGraphResult(
        2, status, PROVIDER_ID, sourceDirs.toString(), ALGORITHM, diagnostics, reachability, edges);
  }

  private static CallGraphResult failed(List<Path> sourceDirs, Throwable error) {
    String diagnostic = error.getClass().getSimpleName() + ": " + error.getMessage();
    StackTraceElement[] stack = error.getStackTrace();
    if (stack.length > 0) diagnostic += "; top_frame=" + stack[0];
    return new CallGraphResult(
        2, "failed", PROVIDER_ID, sourceDirs.toString(), ALGORITHM,
        List.of(diagnostic), List.of(), List.of());
  }
}
