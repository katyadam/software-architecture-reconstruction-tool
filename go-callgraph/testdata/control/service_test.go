package service

import "testing"

func TestDirect(t *testing.T) {
	if Changed() != 1 {
		t.Fatal("changed value")
	}
}
func TestTransitive(t *testing.T) {
	if Transitive() != 1 {
		t.Fatal("changed value")
	}
}
func TestInterface(t *testing.T) {
	if ThroughInterface(Constant{}) != 1 {
		t.Fatal("changed value")
	}
}
func TestUnrelated(t *testing.T) {
	if Untouched() != 9 {
		t.Fatal("unrelated value")
	}
}
