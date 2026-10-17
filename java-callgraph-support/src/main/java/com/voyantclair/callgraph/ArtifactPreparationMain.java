package com.voyantclair.callgraph;

import com.fasterxml.jackson.databind.ObjectMapper;

/** JSON command-line entrypoint for provider-neutral Java artifact preparation. */
public final class ArtifactPreparationMain {
  private static final ObjectMapper JSON = new ObjectMapper();
  private ArtifactPreparationMain() {}
  public static void main(String[] args) throws Exception {
    System.out.println(JSON.writeValueAsString(JavaArtifactPreparer.prepare(AdapterArguments.parse(args).sourceDir())));
  }
}
