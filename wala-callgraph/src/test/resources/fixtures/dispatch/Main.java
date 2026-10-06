interface Worker { void run(); }
class Impl implements Worker { public void run() {} }
public class Main { public static void main(String[] args) { Worker worker = new Impl(); worker.run(); } }
