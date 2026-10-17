package com.voyantclair.callgraph;

import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.file.Path;
import java.util.List;

/** JSON command-line entrypoint for provider-neutral Java artifact preparation. */
public final class ArtifactPreparationMain {
  private static final ObjectMapper JSON = new ObjectMapper();
  private ArtifactPreparationMain() {}
  public static void main(String[] args) throws Exception {
    var artifacts = JavaArtifactPreparer.prepare(AdapterArguments.parse(args).sourceDir());
    System.out.println(JSON.writeValueAsString(new ArtifactResult(
        strings(artifacts.applicationClassDirs()),
        strings(artifacts.testClassDirs()),
        strings(artifacts.classpath()),
        artifacts.diagnostic())));
  }

  private static List<String> strings(List<Path> paths) {
    return paths.stream().map(Path::toString).toList();
  }

  /** Wire format deliberately uses native filesystem strings rather than Jackson's file URIs. */
  private record ArtifactResult(
      List<String> applicationClassDirs,
      List<String> testClassDirs,
      List<String> classpath,
      String diagnostic) {}
}
