package com.voyantclair.wala;

import static org.junit.jupiter.api.Assertions.*;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;

class WalaCallGraphMainTest {
  @Test void reports_no_entrypoints_as_json_result() throws Exception {
    var result = WalaCallGraphMain.run(Path.of("src/test/resources/fixtures/no-entrypoint"));
    assertEquals("no_entrypoints", result.status());
    assertTrue(result.edges().isEmpty());
  }
  @Test void resolves_interface_dispatch() throws Exception {
    var root = Path.of(getClass().getResource("/fixtures/dispatch").toURI());
    var result = WalaCallGraphMain.run(root);
    assertEquals("ok", result.status());
    assertTrue(result.edges().stream().anyMatch(e -> e.caller().declaring_type().contains("LMain") && e.callee().declaring_type().contains("LImpl") && e.callee().member_name().equals("run")), result.edges().toString());
  }
}
