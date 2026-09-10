package io.github.katyadam.sar.wala;

import com.ibm.wala.cast.ir.ssa.AstIRFactory;
import com.ibm.wala.cast.java.client.impl.ZeroOneContainerCFABuilderFactory;
import com.ibm.wala.cast.java.ipa.callgraph.JavaSourceAnalysisScope;
import com.ibm.wala.cast.java.translator.jdt.ecj.ECJClassLoaderFactory;
import com.ibm.wala.classLoader.IClass;
import com.ibm.wala.classLoader.IMethod;
import com.ibm.wala.classLoader.SourceDirectoryTreeModule;
import com.ibm.wala.classLoader.SourceFileModule;
import com.ibm.wala.core.java11.JrtModule;
import com.ibm.wala.ipa.callgraph.AnalysisCacheImpl;
import com.ibm.wala.ipa.callgraph.AnalysisOptions;
import com.ibm.wala.ipa.callgraph.AnalysisScope;
import com.ibm.wala.ipa.callgraph.CGNode;
import com.ibm.wala.ipa.callgraph.CallGraph;
import com.ibm.wala.ipa.callgraph.CallGraphBuilder;
import com.ibm.wala.ipa.callgraph.Entrypoint;
import com.ibm.wala.ipa.callgraph.IAnalysisCacheView;
import com.ibm.wala.ipa.callgraph.impl.DefaultEntrypoint;
import com.ibm.wala.ipa.cha.ClassHierarchyFactory;
import com.ibm.wala.ipa.cha.IClassHierarchy;
import com.ibm.wala.ssa.SymbolTable;
import com.ibm.wala.types.ClassLoaderReference;
import com.ibm.wala.types.Selector;
import com.ibm.wala.types.TypeName;
import com.ibm.wala.types.TypeReference;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.Iterator;
import java.util.List;

/** Builds a 0-1-CFA call graph directly from Java source files using WALA's ECJ frontend. */
public final class SourceCallGraph {
  private SourceCallGraph() {}

  public static void main(String[] args) throws Exception {
    Arguments arguments = Arguments.parse(args);
    CallGraph graph = buildGraph(arguments.sourcePath(), arguments.mainClass());
    String json = toJson(graph, arguments.includePrimordial());

    if (arguments.output() == null) {
      System.out.println(json);
    } else {
      Files.writeString(arguments.output(), json + System.lineSeparator(), StandardCharsets.UTF_8);
      System.err.printf("Wrote %s%n", arguments.output());
    }
  }

  static CallGraph buildGraph(Path sourcePath, String mainClass) throws Exception {
    AnalysisScope scope = new JavaSourceAnalysisScope();
    // JRT modules are the standard-library representation in JDK 9+.  Adding them directly
    // avoids WalaProperties.getJ2SEJarFiles(), which only supports legacy rt.jar layouts.
    for (Module module : ModuleLayer.boot().modules()) {
      scope.addToScope(ClassLoaderReference.Primordial, new JrtModule(module.getName()));
    }

    File source = sourcePath.toFile();
    if (source.isDirectory()) {
      scope.addToScope(JavaSourceAnalysisScope.SOURCE, new SourceDirectoryTreeModule(source));
    } else {
      scope.addToScope(
          JavaSourceAnalysisScope.SOURCE,
          new SourceFileModule(source, source.getName(), null));
    }

    IClassHierarchy hierarchy = ClassHierarchyFactory.make(scope, new ECJClassLoaderFactory(scope.getExclusions()));
    AnalysisOptions options = new AnalysisOptions();
    Iterable<Entrypoint> entrypoints = findMainEntrypoint(hierarchy, mainClass);
    options.setEntrypoints(entrypoints);
    options.getSSAOptions().setDefaultValues(SymbolTable::getDefaultValue);
    options.setReflectionOptions(AnalysisOptions.ReflectionOptions.NONE);

    IAnalysisCacheView cache =
        new AnalysisCacheImpl(AstIRFactory.makeDefaultFactory(), options.getSSAOptions());
    CallGraphBuilder<?> builder = new ZeroOneContainerCFABuilderFactory().make(options, cache, hierarchy);
    return builder.makeCallGraph(options, null);
  }

  private static Iterable<Entrypoint> findMainEntrypoint(IClassHierarchy hierarchy, String mainClass) {
    TypeReference classReference =
        TypeReference.findOrCreate(JavaSourceAnalysisScope.SOURCE, TypeName.string2TypeName(mainClass));
    IClass clazz = hierarchy.lookupClass(classReference);
    if (clazz == null) {
      throw new IllegalArgumentException("Source class was not found: " + mainClass);
    }
    IMethod main = clazz.getMethod(Selector.make("main([Ljava/lang/String;)V"));
    if (main == null) {
      throw new IllegalArgumentException("No public static void main(String[]) found in " + mainClass);
    }
    return Collections.singletonList(new DefaultEntrypoint(main, hierarchy));
  }

  private static String toJson(CallGraph graph, boolean includePrimordial) {
    List<Edge> edges = new ArrayList<>();
    for (CGNode caller : graph) {
      if (!includePrimordial && isPrimordial(caller)) {
        continue;
      }
      Iterator<CGNode> successors = graph.getSuccNodes(caller);
      while (successors.hasNext()) {
        CGNode callee = successors.next();
        if (includePrimordial || !isPrimordial(callee)) {
          edges.add(new Edge(methodId(caller), methodId(callee)));
        }
      }
    }
    edges.sort(Comparator.comparing(Edge::caller).thenComparing(Edge::callee));

    StringBuilder json = new StringBuilder("{\n  \"analysis\": \"WALA Java source 0-1-CFA\",\n  \"edges\": [");
    for (int i = 0; i < edges.size(); i++) {
      Edge edge = edges.get(i);
      json.append("\n    {\"caller\": \"").append(escape(edge.caller())).append("\", \"callee\": \"")
          .append(escape(edge.callee())).append("\"}");
      if (i + 1 < edges.size()) {
        json.append(',');
      }
    }
    return json.append("\n  ]\n}").toString();
  }

  private static boolean isPrimordial(CGNode node) {
    return node.getMethod().getDeclaringClass().getClassLoader().getReference()
        .equals(ClassLoaderReference.Primordial);
  }

  private static String methodId(CGNode node) {
    return node.getMethod().getReference().toString();
  }

  private static String escape(String value) {
    return value.replace("\\", "\\\\").replace("\"", "\\\"");
  }

  private record Edge(String caller, String callee) {}

  private record Arguments(Path sourcePath, String mainClass, Path output, boolean includePrimordial) {
    static Arguments parse(String[] args) {
      Path sourcePath = null;
      String mainClass = null;
      Path output = null;
      boolean includePrimordial = false;
      for (int i = 0; i < args.length; i++) {
        switch (args[i]) {
          case "--source", "--source-dir" -> sourcePath = Path.of(requireValue(args, ++i, args[i - 1]));
          case "--main-class" -> mainClass = normalizeClassName(requireValue(args, ++i, "--main-class"));
          case "--output" -> output = Path.of(requireValue(args, ++i, "--output"));
          case "--include-primordial" -> includePrimordial = true;
          case "--help", "-h" -> {
            System.out.println("Usage: --source-dir <directory|file> --main-class <package.Class> [--output graph.json] [--include-primordial]");
            System.exit(0);
          }
          default -> throw new IllegalArgumentException("Unknown argument: " + args[i]);
        }
      }
      if (sourcePath == null || mainClass == null) {
        throw new IllegalArgumentException("--source-dir and --main-class are required; pass --help for usage.");
      }
      if (!Files.exists(sourcePath)) {
        throw new IllegalArgumentException("Source path does not exist: " + sourcePath);
      }
      return new Arguments(sourcePath, mainClass, output, includePrimordial);
    }

    private static String requireValue(String[] args, int index, String flag) {
      if (index >= args.length) {
        throw new IllegalArgumentException("Missing value for " + flag);
      }
      return args[index];
    }

    private static String normalizeClassName(String className) {
      String normalized = className.replace('.', '/');
      return normalized.startsWith("L") ? normalized : "L" + normalized;
    }
  }
}
