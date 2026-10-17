package control;

public class OverriddenQuote extends BaseQuote {
    @Override
    public int amount() {
        return 10;
    }
}
