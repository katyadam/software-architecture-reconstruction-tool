package com.voyantclair.wala;

import java.nio.file.Path;

/** Prepares bytecode inputs for a conventional Java build-tool source root. */
public interface BytecodeArtifactPreparer {
  /** Returns compiled artifacts or a structured diagnostic for the supplied source root. */
  BytecodeArtifacts.Preparation prepare(Path sourceRoot);
}
