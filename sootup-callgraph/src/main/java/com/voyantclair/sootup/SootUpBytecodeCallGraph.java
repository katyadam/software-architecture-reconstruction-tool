package com.voyantclair.sootup;

import com.voyantclair.callgraph.AdapterArguments;
import com.voyantclair.callgraph.CallGraphResult;
import java.io.File;
import java.util.ArrayList;
import java.util.ArrayDeque;
import java.util.HashSet;
import java.util.List;
import sootup.callgraph.ClassHierarchyAnalysisAlgorithm;
import sootup.core.inputlocation.AnalysisInputLocation;
import sootup.core.signatures.MethodSignature;
import sootup.core.types.Type;
import sootup.java.bytecode.frontend.inputlocation.DefaultRuntimeAnalysisInputLocation;
import sootup.java.bytecode.frontend.inputlocation.JavaClassPathAnalysisInputLocation;
import sootup.java.core.JavaSootMethod;
import sootup.java.core.views.JavaView;

/** Builds a SootUp CHA graph without depending on WALA implementation classes. */
public final class SootUpBytecodeCallGraph {
  private SootUpBytecodeCallGraph() {}

  public static CallGraphResult analyze(AdapterArguments arguments) {
    if (!arguments.algorithm().equals("cha")) {
      throw new IllegalArgumentException("SootUp supports only --algorithm cha");
    }
    var started = System.nanoTime();
    try {
      var paths = new ArrayList<String>();
      arguments.applicationClassDirs().forEach(path -> paths.add(path.toString()));
      arguments.testClassDirs().forEach(path -> paths.add(path.toString()));
      arguments.classpath().forEach(path -> paths.add(path.toString()));
      List<AnalysisInputLocation> locations = List.of(
          new JavaClassPathAnalysisInputLocation(String.join(File.pathSeparator, paths)),
          new DefaultRuntimeAnalysisInputLocation());
      var view = new JavaView(locations);
      var methods = view.getClasses().flatMap(clazz -> clazz.getMethods().stream()).toList();
      var entrypoints = arguments.entrypoints().stream()
          .map(selector -> find(methods, selector))
          .flatMap(java.util.Optional::stream)
          .map(JavaSootMethod::getSignature)
          .toList();
      if (entrypoints.isEmpty()) {
        return result(arguments, List.of("no SootUp entrypoints matched"), List.of());
      }
      System.err.println("callgraph_phase=building provider=sootup-java elapsed_ms=" + elapsed(started));
      var graph = new ClassHierarchyAnalysisAlgorithm(view).initialize(entrypoints);
      var reachability = new ArrayList<CallGraphResult.Reachability>();
      for (var target : arguments.targets()) {
        var matching = graph.getMethodSignatures().stream()
            .filter(method -> targetMatches(target, ref(method))).toList();
        for (var method : matching) {
          reachability.add(new CallGraphResult.Reachability(ref(method), rootsReaching(graph, method, entrypoints)));
        }
      }
      System.err.println("callgraph_phase=ready provider=sootup-java elapsed_ms=" + elapsed(started));
      return result(arguments, List.of("analysis_duration_ms=" + elapsed(started)), reachability);
    } catch (Throwable error) {
      return new CallGraphResult(2, "failed", "sootup-java", arguments.sourceDir().toString(), "cha",
          List.of(error.getClass().getSimpleName() + ": " + error.getMessage()), List.of(), List.of());
    }
  }

  /** Walks callers backwards so each target is associated only with selected roots that reach it. */
  private static List<CallGraphResult.MethodRef> rootsReaching(
      sootup.callgraph.CallGraph graph, MethodSignature target, List<MethodSignature> roots) {
    var remaining = new ArrayDeque<MethodSignature>();
    var visited = new HashSet<MethodSignature>();
    var rootSet = new HashSet<>(roots);
    var reachedRoots = new ArrayList<CallGraphResult.MethodRef>();
    remaining.add(target);
    while (!remaining.isEmpty()) {
      var current = remaining.removeFirst();
      if (!visited.add(current)) continue;
      if (rootSet.contains(current)) reachedRoots.add(ref(current));
      remaining.addAll(graph.callSourcesTo(current));
    }
    return reachedRoots;
  }

  private static CallGraphResult result(AdapterArguments arguments, List<String> diagnostics, List<CallGraphResult.Reachability> reachability) {
    return new CallGraphResult(2, "ok", "sootup-java", arguments.sourceDir().toString(), "cha", diagnostics, reachability, List.of());
  }

  private static java.util.Optional<JavaSootMethod> find(List<JavaSootMethod> methods, String selector) {
    return methods.stream().filter(method -> selector.equals(selector(ref(method)))).findFirst();
  }

  private static boolean targetMatches(String target, CallGraphResult.MethodRef method) {
    var prefix = method.declaring_type() + "#" + method.member_name() + "/";
    return target.startsWith(prefix) && target.substring(prefix.length()).equals(Integer.toString(parameterCount(method.descriptor())));
  }

  private static int parameterCount(String descriptor) {
    var count = 0;
    for (int index = 1; descriptor.charAt(index) != ')'; count++) {
      while (descriptor.charAt(index) == '[') index++;
      if (descriptor.charAt(index) == 'L') index = descriptor.indexOf(';', index) + 1; else index++;
    }
    return count;
  }

  private static String selector(CallGraphResult.MethodRef method) { return method.declaring_type() + "#" + method.member_name() + method.descriptor(); }

  static CallGraphResult.MethodRef ref(JavaSootMethod method) {
    return ref(method.getSignature());
  }

  private static CallGraphResult.MethodRef ref(MethodSignature method) {
    var declaring = "L" + method.getDeclClassType().getFullyQualifiedName().replace('.', '/');
    var name = method.getName().equals("<init>") ? "<init>" : method.getName();
    var descriptor = "(" + method.getParameterTypes().stream().map(SootUpBytecodeCallGraph::descriptor).reduce("", String::concat) + ")" + descriptor(method.getType());
    return new CallGraphResult.MethodRef("java", declaring, name, descriptor, null, null);
  }

  private static String descriptor(Type type) {
    var name = type.toString();
    var dimensions = 0;
    while (name.endsWith("[]")) { dimensions++; name = name.substring(0, name.length() - 2); }
    var base = switch (name) { case "void" -> "V"; case "boolean" -> "Z"; case "byte" -> "B"; case "char" -> "C"; case "short" -> "S"; case "int" -> "I"; case "long" -> "J"; case "float" -> "F"; case "double" -> "D"; default -> "L" + name.replace('.', '/') + ";"; };
    return "[".repeat(dimensions) + base;
  }

  private static long elapsed(long started) { return (System.nanoTime() - started) / 1_000_000; }
}
