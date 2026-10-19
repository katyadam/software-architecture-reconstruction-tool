package com.voyantclair.wala;

import java.nio.file.Path;
import java.util.List;

/** Defines build-tool-neutral bytecode inputs consumed by the WALA backend. */
public final class BytecodeArtifacts {
  private BytecodeArtifacts() {}

  /** Holds compiled application/test directories and resolved dependencies for one module. */
  public record PreparedArtifacts(
      List<Path> applicationClassDirs,
      List<Path> testClassDirs,
      List<Path> dependencyJars,
      Path moduleRoot) {}

  /** Carries prepared artifacts or a diagnostic that requires conservative TIA fallback. */
  public record Preparation(PreparedArtifacts artifacts, String diagnostic) {
    /** Returns whether build-tool preparation produced usable bytecode inputs. */
    public boolean succeeded() {
      return artifacts != null;
    }
  }
}
