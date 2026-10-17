package consumer

import "testing"

func TestCrossPackage(t *testing.T) {
	if Read() != 1 {
		t.Fatal("changed value")
	}
}
