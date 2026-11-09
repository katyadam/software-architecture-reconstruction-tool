package example;
public class ServiceTest {
  public void testChanged() { Service.changed(); Service.overloaded("value"); }
  public void testUnrelated() { Service.overloaded(); }
}
