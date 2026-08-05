package engine

import "testing"

func TestInspectBlocksObviousSqli(t *testing.T) {
	eng, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	resp := eng.Inspect(InspectRequest{
		Method:    "GET",
		Host:      "example.com",
		URI:       "/?id=1' OR '1'='1",
		Headers:   map[string]string{},
		SrcIP:     "1.2.3.4",
		UserAgent: "sqlmap/1.0",
	})
	if !resp.Blocked {
		t.Fatalf("expected blocked, got %+v", resp)
	}
	if resp.Engine != "coraza+crs" {
		t.Fatalf("engine=%s", resp.Engine)
	}
}

func TestInspectAllowsClean(t *testing.T) {
	eng, err := New()
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	resp := eng.Inspect(InspectRequest{
		Method:    "GET",
		Host:      "example.com",
		URI:       "/index.html",
		Headers:   map[string]string{},
		SrcIP:     "1.2.3.4",
		UserAgent: "Mozilla/5.0",
	})
	if resp.Blocked {
		t.Fatalf("expected allow, got %+v", resp)
	}
}
