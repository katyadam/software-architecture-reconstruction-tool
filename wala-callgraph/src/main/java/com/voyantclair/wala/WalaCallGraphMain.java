package com.voyantclair.wala;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.ibm.wala.cast.ir.ssa.AstIRFactory;
import com.ibm.wala.cast.java.client.impl.ZeroOneContainerCFABuilderFactory;
import com.ibm.wala.cast.java.ipa.callgraph.JavaSourceAnalysisScope;
import com.ibm.wala.cast.java.translator.jdt.ecj.ECJClassLoaderFactory;
import com.ibm.wala.classLoader.SourceDirectoryTreeModule;
import com.ibm.wala.ipa.callgraph.*;
import com.ibm.wala.ipa.callgraph.impl.DefaultEntrypoint;
import com.ibm.wala.ipa.cha.*;
import com.ibm.wala.ssa.SymbolTable;
import com.ibm.wala.types.ClassLoaderReference;
import com.ibm.wala.util.config.PatternsFilter;
import java.nio.file.*;
import java.util.*;
import java.util.jar.JarFile;
public final class WalaCallGraphMain {
  private static final ObjectMapper JSON = new ObjectMapper();
  public static CallGraphResult run(Path sourceDir) throws Exception {
    try (var files = Files.walk(sourceDir)) {
      if (files.filter(p -> p.toString().endsWith(".java")).noneMatch(p -> { try { var s = Files.readString(p); return s.contains("static void main") && s.contains("String[]"); } catch (Exception e) { return false; } })) return result("no_entrypoints", sourceDir, List.of());
    }
    var scope = new JavaSourceAnalysisScope();
    scope.setExclusions(PatternsFilter.builder().addAll(WalaCallGraphMain.class.getClassLoader().getResourceAsStream("Java60RegressionExclusions.txt")).build());
    String runtimeJar = System.getProperty("voyantclair.wala.rtJar", System.getenv().getOrDefault("VOYANTCLAIR_WALA_RT_JAR", "/Library/Java/JavaVirtualMachines/temurin-8.jdk/Contents/Home/jre/lib/rt.jar"));
    scope.addToScope(ClassLoaderReference.Primordial, new JarFile(runtimeJar));
    scope.addToScope(JavaSourceAnalysisScope.SOURCE, new SourceDirectoryTreeModule(sourceDir.toFile()));
    IClassHierarchy cha = ClassHierarchyFactory.make(scope, new ECJClassLoaderFactory(scope.getExclusions()));
    var entries = new ArrayList<Entrypoint>();
    for (var c : cha) for (var m : c.getDeclaredMethods()) if (m.isPublic() && m.isStatic() && m.getName().toString().equals("main") && m.getDescriptor().toString().equals("([Ljava/lang/String;)V")) entries.add(new DefaultEntrypoint(m, cha));
    if (entries.isEmpty()) return result("no_entrypoints", sourceDir, List.of());
    var options = new AnalysisOptions(scope, entries); options.getSSAOptions().setDefaultValues(SymbolTable::getDefaultValue);
    var cache = new AnalysisCacheImpl(AstIRFactory.makeDefaultFactory(), options.getSSAOptions());
    var cg = new ZeroOneContainerCFABuilderFactory().make(options, cache, cha).makeCallGraph(options, null);
    var edges = new ArrayList<CallGraphResult.Edge>();
    for (var node : cg) { var successors = cg.getSuccNodes(node); while (successors.hasNext()) { var succ = successors.next(); edges.add(new CallGraphResult.Edge(ref(node.getMethod()), ref(succ.getMethod()), "wala-java", "zero_one_container_cfa", 1f)); } }
    return result("ok", sourceDir, edges);
  }
  private static CallGraphResult.MethodRef ref(com.ibm.wala.classLoader.IMethod m) { return new CallGraphResult.MethodRef("java", m.getDeclaringClass().getName().toString(), m.getName().toString(), m.getDescriptor().toString(), null, null); }
  private static CallGraphResult result(String status, Path root, List<CallGraphResult.Edge> edges) { return new CallGraphResult(1, status, "wala-java", root.toString(), "zero_one_container_cfa", List.of(), edges); }
  public static void main(String[] args) throws Exception { Path root = Path.of(args[1]); System.out.println(JSON.writeValueAsString(run(root))); }
}
