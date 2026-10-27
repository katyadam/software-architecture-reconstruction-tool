package com.voyantclair.spoon;

import com.voyantclair.callgraph.AdapterArguments;
import com.voyantclair.callgraph.CallGraphResult;
import com.fasterxml.jackson.databind.ObjectMapper;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * CLI entry point for the Spoon source-level call graph adapter.
 *
 * <p>Accepts the same {@code AdapterArguments} wire format as the WALA and SootUp adapters.
 * When {@code --source-dir} is provided but no {@code --classes-dir} entries are given, analysis
 * runs in noclasspath mode directly over Java source. When class dirs are present they are ignored
 * (Spoon is source-only); callers should pass {@code --source-dir} as the module source root.
 */
public final class SpoonCallGraphMain {
  private static final ObjectMapper JSON = new ObjectMapper();

  public static void main(String[] args) throws Exception {
    var arguments = AdapterArguments.parse(args);
    var entrypointSelectors = arguments.entrypoints().stream()
        .map(SpoonCallGraphMain::parseMethodSelector)
        .toList();
    var targetSelectors = arguments.targets().stream()
        .map(SpoonCallGraphMain::parseTargetSelector)
        .toList();

    var sourceDirs = new ArrayList<Path>();
    sourceDirs.add(arguments.sourceDir());

    CallGraphResult result = SpoonCallGraph.analyze(
        sourceDirs,
        arguments.classpath(),
        entrypointSelectors,
        targetSelectors);

    System.out.println(JSON.writeValueAsString(result));
  }

  /** Parses a {@code Lpackage/Class#method(descriptor)} entrypoint selector. */
  static SpoonCallGraph.MethodSelector parseMethodSelector(String value) {
    int hash = value.indexOf('#');
    int openParen = value.indexOf('(', hash + 1);
    int closeParen = value.indexOf(')', openParen + 1);
    if (hash <= 0 || openParen <= hash + 1 || closeParen < openParen) {
      throw new IllegalArgumentException("Invalid --entrypoint selector: " + value);
    }
    return new SpoonCallGraph.MethodSelector(
        value.substring(0, hash),
        value.substring(hash + 1, openParen),
        value.substring(openParen));
  }

  /** Parses a {@code Lpackage/Class#method/arity} target selector. */
  static SpoonCallGraph.TargetSelector parseTargetSelector(String value) {
    int hash = value.indexOf('#');
    int slash = value.lastIndexOf('/');
    if (hash <= 0 || slash <= hash + 1 || slash == value.length() - 1) {
      throw new IllegalArgumentException("Invalid --target selector: " + value);
    }
    return new SpoonCallGraph.TargetSelector(
        value.substring(0, hash),
        value.substring(hash + 1, slash),
        Integer.parseInt(value.substring(slash + 1)));
  }
}
