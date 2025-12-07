import java.util.Objects;

public class UserAccount implements Identifiable {

    private final String id;
    private String username;
    private boolean active;

    public UserAccount(String id, String username, boolean active) {
        this.id = id;
        this.username = username;
        this.active = active;
    }

    // --- interface methods ---
    @Override
    public String getId() {
        return id;
    }

    @Override
    public boolean isActive() {
        return active;
    }

    // --- regular methods ---
    public void deactivate() {
        this.active = false;
    }

    public void changeUsername(String newName) {
        this.username = newName;
    }

    public String getUsername() {
        return username;
    }

    // --- Object overrides ---
    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof UserAccount)) return false;
        UserAccount that = (UserAccount) o;
        return active == that.active &&
                Objects.equals(id, that.id) &&
                Objects.equals(username, that.username);
    }

    @Override
    public int hashCode() {
        return Objects.hash(id, username, active);
    }

    @Override
    public String toString() {
        return "UserAccount{" +
                "id='" + id + '\'' +
                ", username='" + username + '\'' +
                ", active=" + active +
                '}';
    }
}
