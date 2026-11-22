package weft

import (
	"net/url"
	"regexp"
	"strconv"
	"strings"
)

var (
	emailRe = regexp.MustCompile("^[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?(?:\\.[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?)+$")
	uriRe   = regexp.MustCompile(`^[A-Za-z][A-Za-z0-9+.-]*:\S*$`)
	labelRe = regexp.MustCompile(`^[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?$`)
	dateRe  = regexp.MustCompile(`^(\d{4})-(\d{2})-(\d{2})$`)
	timeRe  = regexp.MustCompile(`^([01]\d|2[0-3]):([0-5]\d):([0-5]\d)(\.\d+)?(Z|z|[+-](?:[01]\d|2[0-3]):[0-5]\d)?$`)
	hexRe   = regexp.MustCompile(`^[0-9A-Fa-f]{1,4}$`)
	decRe   = regexp.MustCompile(`^(0|[1-9]\d{0,2})$`)
)

// isRealDate reports whether year-month-day is a real calendar day.
func isRealDate(year, month, day int) bool {
	if month < 1 || month > 12 || day < 1 {
		return false
	}
	return day <= daysIn(year, month)
}

func daysIn(year, month int) int {
	switch month {
	case 2:
		if year%4 == 0 && (year%100 != 0 || year%400 == 0) {
			return 29
		}
		return 28
	case 4, 6, 9, 11:
		return 30
	}
	return 31
}

func isIPv4(s string) bool {
	parts := strings.Split(s, ".")
	if len(parts) != 4 {
		return false
	}
	for _, p := range parts {
		if !decRe.MatchString(p) {
			return false
		}
		n, _ := strconv.Atoi(p)
		if n > 255 {
			return false
		}
	}
	return true
}

func isIPv6(s string) bool {
	for _, r := range s {
		if !(r == ':' || r == '.' || (r >= '0' && r <= '9') || (r >= 'a' && r <= 'f') || (r >= 'A' && r <= 'F')) {
			return false
		}
	}
	halves := strings.Split(s, "::")
	if len(halves) > 2 {
		return false
	}
	split := func(part string) []string {
		if part == "" {
			return nil
		}
		return strings.Split(part, ":")
	}
	head := split(halves[0])
	var rest []string
	compressed := len(halves) == 2
	if compressed {
		rest = split(halves[1])
	}
	// An embedded IPv4 address is only allowed as the very last group.
	tail := head
	if compressed {
		tail = rest
	}
	groups := 0
	for _, list := range [][]string{head, rest} {
		isTail := len(list) > 0 && len(tail) > 0 && &list[0] == &tail[0]
		for i, g := range list {
			if strings.Contains(g, ".") {
				if !isTail || i != len(list)-1 || !isIPv4(g) {
					return false
				}
				groups += 2
				continue
			}
			if !hexRe.MatchString(g) {
				return false
			}
			groups++
		}
	}
	if compressed {
		return groups <= 7
	}
	return groups == 8
}

func isHostname(s string) bool {
	if len(s) == 0 || len(s) > 253 {
		return false
	}
	for _, label := range strings.Split(s, ".") {
		if !labelRe.MatchString(label) {
			return false
		}
	}
	return true
}

func isURL(s string) bool {
	u, err := url.Parse(s)
	if err != nil {
		return false
	}
	return (u.Scheme == "http" || u.Scheme == "https") && u.Hostname() != ""
}

func isDate(s string) bool {
	m := dateRe.FindStringSubmatch(s)
	if m == nil {
		return false
	}
	y, _ := strconv.Atoi(m[1])
	mo, _ := strconv.Atoi(m[2])
	d, _ := strconv.Atoi(m[3])
	return isRealDate(y, mo, d)
}

// CheckFormat reports whether value is valid for the @format named format (spec/behaviour.md).
func CheckFormat(format, value string) bool {
	switch format {
	case "email":
		return len(value) <= 254 && emailRe.MatchString(value)
	case "url":
		return isURL(value)
	case "uri":
		return uriRe.MatchString(value)
	case "hostname":
		return isHostname(value)
	case "ipv4":
		return isIPv4(value)
	case "ipv6":
		return isIPv6(value)
	case "date":
		return isDate(value)
	case "time":
		return timeRe.MatchString(value)
	}
	return false
}
