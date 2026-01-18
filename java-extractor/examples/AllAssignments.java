package com.example.assignments;

import java.util.Arrays;

public class AllAssignments {

    static int staticCount = 0;

    private int instanceValue = 10;
    private String name;

    private int[] numbers = new int[5];

    public AllAssignments(String name) {
        this.name = name;
    }


    public void demoAssignments() {
        int a;
        a = 5;

        int b = 10;

        int x = 1, y = 2;

        int c, d;
        c = d = 15;

        a += 3;   // a = a + 3
        b *= 2;

        numbers[0] = 42;
        numbers[1] += 10;

        AssignmentDemo obj1 = this;
        AssignmentDemo obj2 = obj1;

        double pi = 3.1415;
        int intPi = (int) pi;

        long big = a;

        byte small = (byte) b;

        java.util.Date date = new java.util.Date();

        this.instanceValue = 50;

        AssignmentDemo.staticCount = 200;

        System.out.println("a=" + a + ", b=" + b + ", numbers=" + Arrays.toString(numbers));
    }

    public static void main(String[] args) {
        AssignmentDemo demo = new AssignmentDemo("Test");
        demo.demoAssignments();
    }
}
