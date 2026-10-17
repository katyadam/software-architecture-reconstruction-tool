package com.voyantclair.sootup;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.voyantclair.callgraph.AdapterArguments;

/** Standalone schema-v2 SootUp adapter entrypoint. */
public final class SootUpCallGraphMain {
  private static final ObjectMapper JSON = new ObjectMapper();
  private SootUpCallGraphMain() {}
  public static void main(String[] args) throws Exception {
    System.out.println(JSON.writeValueAsString(SootUpBytecodeCallGraph.analyze(AdapterArguments.parse(args))));
  }
}
