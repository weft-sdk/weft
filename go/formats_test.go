package weft_test

import (
	"strings"
	"testing"

	weft "github.com/weft-sdk/weft/go"
)

func TestFormats(t *testing.T) {
	cases := map[string]struct{ ok, bad []string }{
		"email":    {[]string{"a@example.com", "first.last+tag@sub.example.co"}, []string{"", "a@b", "a b@example.com", "@example.com", "a@@example.com", "a@.com"}},
		"url":      {[]string{"https://example.com", "http://example.com:8080/a?b=c#d"}, []string{"example.com", "ftp://example.com", "mailto:a@b.co", "https://", ""}},
		"uri":      {[]string{"urn:isbn:0451450523", "mailto:a@b.co", "https://example.com"}, []string{"example.com", "://x", "a b:c", ""}},
		"hostname": {[]string{"example.com", "a", "a-b.example.org", "xn--nxasmq6b.example"}, []string{"", "-a.com", "a-.com", "a..com", "a_b.com", strings.Repeat("a", 64) + ".com"}},
		"ipv4":     {[]string{"0.0.0.0", "192.168.1.255", "10.0.0.1"}, []string{"256.1.1.1", "1.2.3", "1.2.3.4.5", "01.2.3.4", "a.b.c.d", ""}},
		"ipv6": {
			[]string{"::1", "::", "2001:db8::8a2e:370:7334", "1:2:3:4:5:6:7:8", "::ffff:192.0.2.1", "1::"},
			[]string{"", ":::", "1:2:3:4:5:6:7", "1:2:3:4:5:6:7:8:9", "12345::", "1::2::3", "::1.2.3", "1.2.3.4::", "g::1"},
		},
		"date": {[]string{"2026-11-23", "2024-02-29", "2000-02-29"}, []string{"2025-02-29", "1900-02-29", "2026-13-01", "2026-00-10", "2026-11-31", "26-11-23", "2026-1-1", ""}},
		"time": {[]string{"10:00:00", "23:59:59", "10:00:00.123", "10:00:00Z", "10:00:00+02:00"}, []string{"24:00:00", "10:60:00", "10:00", "10:00:00+2", ""}},
	}
	for format, c := range cases {
		for _, v := range c.ok {
			if !weft.CheckFormat(format, v) {
				t.Errorf("%s should accept %q", format, v)
			}
		}
		for _, v := range c.bad {
			if weft.CheckFormat(format, v) {
				t.Errorf("%s should reject %q", format, v)
			}
		}
	}
	if weft.CheckFormat("nope", "x") {
		t.Error("an unknown format accepts nothing")
	}
}
