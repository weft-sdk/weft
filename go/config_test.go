package weft_test

import (
	"os"
	"path/filepath"
	"testing"

	weft "github.com/weft-sdk/weft/go"
)

func TestConfigHelpers(t *testing.T) {
	c, err := weft.ParseConfig("service: a\ntopics:\n  order.placed: prod.placed\nservices:\n  api: http://api", nil)
	if err != nil {
		t.Fatal(err)
	}
	if c.TopicName("order.placed") != "prod.placed" || c.TopicName("other") != "other" {
		t.Error("TopicName")
	}
	if u, ok := c.ServiceURL("api"); !ok || u != "http://api" {
		t.Error("ServiceURL")
	}
	if _, ok := c.ServiceURL("nope"); ok {
		t.Error("an unknown service has no URL")
	}
}

func TestLoadConfigFindsTheFileInAFolderAbove(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "weft.yaml"), []byte("service: found"), 0o644); err != nil {
		t.Fatal(err)
	}
	deep := filepath.Join(dir, "a", "b")
	if err := os.MkdirAll(deep, 0o755); err != nil {
		t.Fatal(err)
	}
	if path, ok := weft.FindConfigFile(deep); !ok || path != filepath.Join(dir, "weft.yaml") {
		t.Errorf("path = %q %v", path, ok)
	}
	c, err := weft.LoadConfig(filepath.Join(dir, "weft.yaml"))
	if err != nil || c.Service != "found" {
		t.Errorf("%v %v", c, err)
	}
	if _, err := weft.LoadConfig(filepath.Join(dir, "missing.yaml")); err == nil {
		t.Error("a missing file is an error")
	}
	var ce *weft.ConfigError
	_, err = weft.LoadConfig(filepath.Join(dir, "missing.yaml"))
	if !asConfigError(err, &ce) || len(ce.Problems) != 1 {
		t.Errorf("err = %v", err)
	}
}

func asConfigError(err error, target **weft.ConfigError) bool {
	if e, ok := err.(*weft.ConfigError); ok {
		*target = e
		return true
	}
	return false
}
