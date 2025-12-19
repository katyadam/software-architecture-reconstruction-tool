// ----- PACKAGE -----
package com.example.importstyles;

// ----- SINGLE-TYPE IMPORTS -----
import java.util.List;                 // import one class
import java.io.File;                  // import one class

// ----- ON-DEMAND / WILDCARD IMPORTS -----
import java.util.*;                   // import all types from a package
import java.time.*;                   // wildcard import from another package

// ----- STATIC SINGLE-MEMBER IMPORTS -----
import static java.lang.Math.PI;      // import one static field
import static java.lang.Math.sqrt;    // import one static method

// ----- STATIC ON-DEMAND / WILDCARD IMPORTS -----
import static java.lang.Math.*;       // import all static members

// ----- IMPORTING NESTED/INNER CLASSES -----
import java.util.Map.Entry;           // inner class import

// ----- FULLY-QUALIFIED TYPE (NO import at all) -----
// (Used directly in code)

public class ImportExamples {

    public static void main(String[] args) {

        // Using a single-type import
        List<String> list = List.of("a", "b");

        // Using a wildcard import
        LocalDate date = LocalDate.now();

        // Using static single-member imports
        double circumference = 2 * PI * 10;

        // Using static wildcard import
        double value = abs(-5); // Math.abs

        // Using explicit nested class import
        Entry<String, Integer> entry = Map.of("key", 42).entrySet().iterator().next();

        // Using a fully-qualified name (no import)
        java.net.URL url;
    }
}