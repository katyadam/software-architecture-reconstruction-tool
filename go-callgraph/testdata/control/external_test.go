package service_test

import (
	service "example.com/tia"
	"testing"
)

func TestExternal(t *testing.T) {
	if service.Changed() != 1 {
		t.Fatal("changed value")
	}
}
