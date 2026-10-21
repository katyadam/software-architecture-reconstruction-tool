package independent

import "testing"

func TestIndependent(t *testing.T) {
	if 2+2 != 4 {
		t.Fatal("unexpected result")
	}
}
