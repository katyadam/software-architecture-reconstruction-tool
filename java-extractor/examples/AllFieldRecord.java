package com.java.test;

import java.util.List;
import java.util.Map;

public record AllFieldRecord(
        String name,               // public getter: name()
        int age,                   // public getter: age()
        boolean active             // public getter: active()
){

    // Static fields (allowed)
    public static final double VERSION = 1.0;
    private static int counter;
    protected static volatile boolean runningFlag;
    static transient String tempStatic;

    // Constructor customization (canonical constructor)
    public AllFieldRecord {
        if (age < 0) {
            throw new IllegalArgumentException("Age cannot be negative");
        }
    }

    // Custom method
    public String greet() {
        return "Hello, " + name;
    }

    // Static method
    public static void incrementCounter() {
        counter++;
    }
}
