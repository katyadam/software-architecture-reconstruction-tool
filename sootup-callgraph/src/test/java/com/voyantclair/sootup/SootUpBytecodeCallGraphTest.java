package com.voyantclair.sootup;

import static org.junit.jupiter.api.Assertions.assertEquals;

import com.voyantclair.callgraph.AdapterArguments;
import java.net.URI;
import java.nio.file.Path;
import java.util.List;
import javax.tools.ToolProvider;
import org.junit.jupiter.api.Test;

class SootUpBytecodeCallGraphTest {
  @Test
  void reports_selected_test_reachability_and_exact_overload_descriptors() throws Exception {
    var root = Path.of(resource("fixtures/direct-call")).toAbsolutePath();
    var output = root.resolveSibling("classes");
    var compiler = ToolProvider.getSystemJavaCompiler();
    assertEquals(0, compiler.run(null, null, null, "-d", output.toString(), root.resolve("src/main/java/example/Service.java").toString(), root.resolve("src/test/java/example/ServiceTest.java").toString()));
    var result = SootUpBytecodeCallGraph.analyze(AdapterArguments.parse(new String[] {
      "--source-dir", root.resolve("src/main/java").toString(), "--classes-dir", output.toString(), "--test-classes-dir", output.toString(),
      "--entrypoint", "Lexample/ServiceTest#testChanged()V", "--target", "Lexample/Service#changed/0", "--target", "Lexample/Service#overloaded/1", "--algorithm", "cha" }));
    assertEquals("ok", result.status());
    assertEquals(2, result.reachability().size());
    assertEquals("()V", result.reachability().get(0).entrypoints().get(0).descriptor());
  }

  @Test
  void emits_empty_reachability_for_unreached_target() throws Exception {
    var root = Path.of(resource("fixtures/direct-call")).toAbsolutePath();
    var output = root.resolveSibling("classes-empty");
    assertEquals(0, ToolProvider.getSystemJavaCompiler().run(null, null, null, "-d", output.toString(), root.resolve("src/main/java/example/Service.java").toString(), root.resolve("src/test/java/example/ServiceTest.java").toString()));
    var result = SootUpBytecodeCallGraph.analyze(AdapterArguments.parse(new String[] {"--source-dir", root.resolve("src/main/java").toString(), "--classes-dir", output.toString(), "--entrypoint", "Lexample/ServiceTest#testChanged()V", "--target", "Lexample/Service#unchanged/0"}));
    assertEquals(List.of(), result.reachability());
  }

  private static URI resource(String name) throws Exception { return SootUpBytecodeCallGraphTest.class.getClassLoader().getResource(name).toURI(); }
}
