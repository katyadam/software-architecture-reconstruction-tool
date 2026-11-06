package control;

public class InjectedQuote {
    private final int amount;

    public InjectedQuote(int amount) {
        this.amount = amount;
    }

    public int amount() {
        return amount;
    }
}
