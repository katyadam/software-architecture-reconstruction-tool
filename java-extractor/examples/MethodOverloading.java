package com.example.overloading;

public class Calculator {

    public int add(int a, int b) {
        return a + b;
    }

    public double add(double a, double b) {
        return a + b;
    }

    public int add(int a, int b, int c) {
        return a + b + c;
    }

    public String add(String a, String b) {
        return a + b;
    }

    public void demo() {
        int a = 2;
        int b = 3;
        int sum1 = add(a, b);
        double sum2 = add(2.5, 3.5);
        int sum3 = add(1, 2, 3);
        String sum4 = add("Hello, ", "World!");
    }

    public static void main(String[] args) {
        Calculator calculator = new Calculator();
        calculator.demo();
    }
}
