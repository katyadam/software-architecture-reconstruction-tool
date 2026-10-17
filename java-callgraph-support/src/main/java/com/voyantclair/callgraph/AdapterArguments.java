package com.voyantclair.callgraph;

import java.io.File;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** Parses the backend-neutral command line shared by standalone Java call-graph adapters. */
public record AdapterArguments(
    Path sourceDir,
    List<Path> applicationClassDirs,
    List<Path> testClassDirs,
    List<Path> classpath,
    List<String> entrypoints,
    List<String> targets,
    String algorithm) {
  public static AdapterArguments parse(String[] arguments) {
    Path sourceDir = null;
    var applicationClassDirs = new ArrayList<Path>();
    var testClassDirs = new ArrayList<Path>();
    var classpath = new ArrayList<Path>();
    var entrypoints = new ArrayList<String>();
    var targets = new ArrayList<String>();
    var algorithm = "cha";
    for (int index = 0; index < arguments.length; index++) {
      var argument = arguments[index];
      if (argument.equals("--source-dir")) {
        sourceDir = Path.of(value(arguments, ++index, argument));
      } else if (argument.equals("--classes-dir")) {
        applicationClassDirs.add(Path.of(value(arguments, ++index, argument)));
      } else if (argument.equals("--test-classes-dir")) {
        testClassDirs.add(Path.of(value(arguments, ++index, argument)));
      } else if (argument.equals("--classpath")) {
        for (var entry : value(arguments, ++index, argument).split(java.util.regex.Pattern.quote(File.pathSeparator))) {
          if (!entry.isBlank()) {
            classpath.add(Path.of(entry));
          }
        }
      } else if (argument.equals("--entrypoint")) {
        entrypoints.add(value(arguments, ++index, argument));
      } else if (argument.equals("--target")) {
        targets.add(value(arguments, ++index, argument));
      } else if (argument.equals("--algorithm")) {
        algorithm = value(arguments, ++index, argument);
      }
    }
    if (sourceDir == null) {
      throw new IllegalArgumentException("Missing required --source-dir argument");
    }
    return new AdapterArguments(
        sourceDir,
        List.copyOf(applicationClassDirs),
        List.copyOf(testClassDirs),
        List.copyOf(classpath),
        List.copyOf(entrypoints),
        List.copyOf(targets),
        algorithm);
  }

  private static String value(String[] arguments, int index, String option) {
    if (index >= arguments.length) {
      throw new IllegalArgumentException("Missing value for " + option);
    }
    return arguments[index];
  }
}
