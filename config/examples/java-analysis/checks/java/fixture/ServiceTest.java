package fixture;

public class ServiceTest {
    @org.junit.jupiter.api.Test
    public void coversChanged() {
        if (Service.changed() != 1) throw new AssertionError();
    }

    @org.junit.jupiter.api.Test
    public void coversUnchanged() {
        if (Service.unchanged() != 7) throw new AssertionError();
    }
}
