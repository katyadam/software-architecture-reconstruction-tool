package com.voyantclair.callgraph;

import static org.junit.jupiter.api.Assertions.*;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class JavaArtifactPreparerTest {
  @Test
  void uses_settings_root_for_system_gradle_library_without_tests(@TempDir Path root) throws Exception {
    var module = root.resolve("library");
    var source = module.resolve("src/main/java");
    var classes = module.resolve("build/classes/java/main");
    Files.createDirectories(source);
    Files.createDirectories(classes);
    Files.writeString(root.resolve("settings.gradle"), "include ':library'");
    Files.writeString(module.resolve("build.gradle"), "plugins { id 'java' }");
    var command = root.resolve("fake-gradle");
    Files.writeString(command, "#!/bin/sh\n[ \"$PWD\" -ef '" + root + "' ] || { echo 'wrong Gradle root'; exit 1; }\nprintf 'VOYANTCLAIR_PROJECT=:library\\t" + module + "\\n'\necho 'VOYANTCLAIR_MAIN=" + classes + "'\necho 'VOYANTCLAIR_TEST='\necho 'VOYANTCLAIR_TEST_RUNTIME='\n");
    command.toFile().setExecutable(true);
    System.setProperty("voyantclair.gradle.command", command.toString());
    try {
      var result = JavaArtifactPreparer.prepare(source);
      assertTrue(result.succeeded(), result.diagnostic());
      assertEquals(java.util.List.of(classes), result.applicationClassDirs());
      assertTrue(result.testClassDirs().isEmpty());
    } finally {
      System.clearProperty("voyantclair.gradle.command");
    }
  }
}
