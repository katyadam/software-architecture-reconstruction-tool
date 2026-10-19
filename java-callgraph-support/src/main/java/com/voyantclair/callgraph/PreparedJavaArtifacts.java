package com.voyantclair.callgraph;

import java.nio.file.Path;
import java.util.List;

/** Backend-independent bytecode inputs prepared from one Java build module. */
public record PreparedJavaArtifacts(
    List<Path> applicationClassDirs,
    List<Path> testClassDirs,
    List<Path> classpath,
    Path moduleRoot,
    String diagnostic) {
  public boolean succeeded() {
    return diagnostic == null || diagnostic.isBlank();
  }

  static PreparedJavaArtifacts failure(String diagnostic) {
    return new PreparedJavaArtifacts(List.of(), List.of(), List.of(), null, diagnostic);
  }
}
