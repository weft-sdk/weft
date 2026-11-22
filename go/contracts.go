package weft

import (
	"encoding/base64"
	"encoding/json"
	"fmt"
	"math"
	"math/big"
	"regexp"
	"strconv"
	"strings"
	"sync"
	"time"
	"unicode/utf8"
)

// The code form of a value, which handlers see, in this runtime's dynamic representation:
//
//	string, uuid, decimal, enum    string
//	bool                           bool
//	int, int32, int64              int64
//	float, float32, float64        float64
//	timestamp                      time.Time
//	bytes                          []byte
//	list                           []any
//	map                            map[string]any
//	record                         map[string]any, keyed by field name
//
// The wire form is what encoding/json produces with UseNumber: the same tree, with the wire
// names of records, timestamps as RFC 3339 text, bytes as base64, and so on (spec/behaviour.md).

// Contracts holds the checked contracts of one or more packages, and the one validator and codec
// that read them. Decode turns wire form into code form; Encode does the reverse. Both validate
// and collect every problem. A Contracts is safe for concurrent use.
type Contracts struct {
	packages []Ir
	types    map[string]*TypeDef
	errors   map[string]*ErrorDef
}

type mode int

const (
	encode mode = iota
	decode
)

var (
	uuidRe      = regexp.MustCompile(`^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$`)
	decimalRe   = regexp.MustCompile(`^-?\d+(\.\d+)?$`)
	wholeRe     = regexp.MustCompile(`^-?\d+$`)
	base64Re    = regexp.MustCompile(`^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$`)
	timestampRe = regexp.MustCompile(`^(\d{4})-(\d{2})-(\d{2})[Tt](\d{2}):(\d{2}):(\d{2})(\.\d+)?([Zz]|[+-]\d{2}:\d{2})$`)
)

const maxSafe = 1<<53 - 1

// NewContracts reads the contracts of every package in irs.
func NewContracts(irs ...Ir) *Contracts {
	c := &Contracts{packages: irs, types: map[string]*TypeDef{}, errors: map[string]*ErrorDef{}}
	for i := range irs {
		for j := range irs[i].Types {
			c.types[irs[i].Package+"."+irs[i].Types[j].Name] = &irs[i].Types[j]
		}
		for j := range irs[i].Errors {
			c.errors[irs[i].Package+"."+irs[i].Errors[j].Name] = &irs[i].Errors[j]
		}
	}
	return c
}

// Package returns the contracts of one package, if it was loaded.
func (c *Contracts) Package(name string) (*Ir, bool) {
	for i := range c.packages {
		if c.packages[i].Package == name {
			return &c.packages[i], true
		}
	}
	return nil, false
}

// Validate returns the problems with a value in code form. Empty when it is valid.
func (c *Contracts) Validate(ref TypeRef, value any) []Issue {
	return c.run(ref, value, encode).issues
}

// Decode turns a wire value into code form. When it is not valid the error is InvalidInput and
// lists every issue.
func (c *Contracts) Decode(ref TypeRef, wire any) (any, error) {
	return c.finish(c.run(ref, wire, decode))
}

// Encode turns a value in code form into wire form. When it is not valid the error is
// InvalidInput and lists every issue.
func (c *Contracts) Encode(ref TypeRef, value any) (any, error) {
	return c.finish(c.run(ref, value, encode))
}

// DecodeJSON decodes JSON text. Text that is not JSON is InvalidInput.
func (c *Contracts) DecodeJSON(ref TypeRef, text []byte) (any, error) {
	wire, err := ParseJSON(text)
	if err != nil {
		return nil, &Error{Category: InvalidInput, Message: "Invalid input: the body is not valid JSON", Cause: err}
	}
	return c.Decode(ref, wire)
}

// EncodeJSON encodes a value and writes it as JSON.
func (c *Contracts) EncodeJSON(ref TypeRef, value any) ([]byte, error) {
	wire, err := c.Encode(ref, value)
	if err != nil {
		return nil, err
	}
	return json.Marshal(wire)
}

// ParseJSON parses JSON keeping numbers exact (as json.Number), the form Decode expects.
func ParseJSON(text []byte) (any, error) {
	dec := json.NewDecoder(strings.NewReader(string(text)))
	dec.UseNumber()
	var v any
	if err := dec.Decode(&v); err != nil {
		return nil, err
	}
	if dec.More() {
		return nil, fmt.Errorf("more than one JSON value")
	}
	return v, nil
}

type outcome struct {
	value  any
	issues []Issue
}

func (c *Contracts) finish(o outcome) (any, error) {
	if len(o.issues) > 0 {
		return nil, invalidInput(o.issues)
	}
	return o.value, nil
}

func (c *Contracts) run(ref TypeRef, value any, m mode) outcome {
	w := &walker{c: c, m: m}
	out := w.convert(ref, value, "", nil)
	return outcome{value: out, issues: w.issues}
}

// ---- the walker ----

type walker struct {
	c      *Contracts
	m      mode
	issues []Issue
}

type env map[string]TypeRef

func join(path, key string) string {
	if path == "" {
		return key
	}
	return path + "." + key
}

func (w *walker) fail(path, format string, args ...any) {
	w.issues = append(w.issues, Issue{Path: path, Message: fmt.Sprintf(format, args...)})
}

func describe(v any) string {
	switch v.(type) {
	case nil:
		return "null"
	case []any:
		return "an array"
	case map[string]any:
		return "an object"
	case string:
		return "a string"
	case bool:
		return "a boolean"
	case json.Number, float64, int, int64:
		return "a number"
	case time.Time:
		return "a time"
	case []byte:
		return "bytes"
	}
	return fmt.Sprintf("a %T", v)
}

func (w *walker) subst(ref TypeRef, e env) TypeRef {
	switch ref.Kind {
	case "param":
		if bound, ok := e[ref.Name]; ok {
			return bound
		}
	case "list":
		item := w.subst(*ref.Item, e)
		return TypeRef{Kind: "list", Item: &item}
	case "map":
		k, v := w.subst(*ref.Key, e), w.subst(*ref.Value, e)
		return TypeRef{Kind: "map", Key: &k, Value: &v}
	case "named":
		if len(ref.Args) > 0 {
			args := make([]TypeRef, len(ref.Args))
			for i, a := range ref.Args {
				args[i] = w.subst(a, e)
			}
			ref.Args = args
		}
	}
	return ref
}

func (w *walker) convert(ref TypeRef, value any, path string, e env) any {
	switch ref.Kind {
	case "primitive":
		return w.primitive(ref.Name, value, path)
	case "param":
		bound, ok := e[ref.Name]
		if !ok {
			w.fail(path, "the type parameter %s has no argument", ref.Name)
			return nil
		}
		return w.convert(bound, value, path, nil)
	case "list":
		list, ok := value.([]any)
		if !ok {
			w.fail(path, "must be an array, not %s", describe(value))
			return nil
		}
		out := make([]any, len(list))
		for i, item := range list {
			out[i] = w.convert(*ref.Item, item, fmt.Sprintf("%s[%d]", path, i), e)
		}
		return out
	case "map":
		obj, ok := value.(map[string]any)
		if !ok {
			w.fail(path, "must be an object, not %s", describe(value))
			return nil
		}
		keyRef := w.subst(*ref.Key, e)
		out := make(map[string]any, len(obj))
		for k, v := range obj {
			keyPath := fmt.Sprintf("%s[%s]", path, k)
			saved := w.m
			w.m = decode
			w.convert(keyRef, k, keyPath, e)
			w.m = saved
			out[k] = w.convert(*ref.Value, v, keyPath, e)
		}
		return out
	case "named":
		def, ok := w.c.types[ref.Package+"."+ref.Name]
		if !ok {
			w.fail(path, "the contracts have no type %s.%s", ref.Package, ref.Name)
			return nil
		}
		if def.Kind == "enum" {
			names := make([]string, len(def.Variants))
			for i, v := range def.Variants {
				names[i] = v.Name
			}
			s, isString := value.(string)
			if !isString {
				w.fail(path, "must be one of %s", strings.Join(names, ", "))
				return nil
			}
			for _, n := range names {
				if n == s {
					return s
				}
			}
			w.fail(path, "%q is not one of %s", s, strings.Join(names, ", "))
			return nil
		}
		inner := env{}
		for i, p := range def.Params {
			if i < len(ref.Args) {
				inner[p] = w.subst(ref.Args[i], e)
			}
		}
		return w.fields(def.Fields, value, path, inner)
	}
	w.fail(path, "unknown type kind %q", ref.Kind)
	return nil
}

func (w *walker) fields(fields []FieldDef, value any, path string, e env) any {
	obj, ok := value.(map[string]any)
	if !ok {
		w.fail(path, "must be an object, not %s", describe(value))
		return nil
	}
	out := make(map[string]any, len(fields))
	for _, f := range fields {
		from, to := f.Name, f.WireName
		if w.m == decode {
			from, to = f.WireName, f.Name
		}
		fieldPath := join(path, from)
		raw, present := obj[from]
		if !present || raw == nil {
			if !f.Optional {
				w.fail(fieldPath, "is required")
			} else if present && w.m == decode {
				out[to] = nil
			}
			continue
		}
		before := len(w.issues)
		converted := w.convert(w.subst(f.Type, e), raw, fieldPath, e)
		if len(w.issues) == before {
			// Rules read the code form: what was decoded, or what the caller gave to encode.
			codeForm := converted
			if w.m == encode {
				codeForm = raw
			}
			w.rules(f.Rules, codeForm, fieldPath)
		}
		out[to] = converted
	}
	return out
}

// ---- primitives ----

func (w *walker) primitive(name string, value any, path string) any {
	switch name {
	case "string":
		if s, ok := value.(string); ok {
			return s
		}
		w.fail(path, "must be a string, not %s", describe(value))
	case "bool":
		if b, ok := value.(bool); ok {
			return b
		}
		w.fail(path, "must be a boolean, not %s", describe(value))
	case "float", "float32", "float64":
		if f, ok := toFloat(value); ok && !math.IsInf(f, 0) && !math.IsNaN(f) {
			return f
		}
		w.fail(path, "must be a finite number, not %s", describe(value))
	case "decimal":
		if s, ok := value.(string); ok && decimalRe.MatchString(s) {
			return s
		}
		w.fail(path, `must be a decimal string such as "12.50"`)
	case "uuid":
		if s, ok := value.(string); ok && uuidRe.MatchString(s) {
			return s
		}
		w.fail(path, "must be a UUID")
	case "int", "int32", "int64":
		return w.integer(name, value, path)
	case "timestamp":
		return w.timestamp(value, path)
	case "bytes":
		if w.m == encode {
			if b, ok := value.([]byte); ok {
				return base64.StdEncoding.EncodeToString(b)
			}
			w.fail(path, "must be bytes, not %s", describe(value))
			return nil
		}
		if s, ok := value.(string); ok && base64Re.MatchString(s) {
			b, err := base64.StdEncoding.DecodeString(s)
			if err == nil {
				return b
			}
		}
		w.fail(path, "must be a base64 string")
	default:
		w.fail(path, "unknown primitive %q", name)
	}
	return nil
}

// toFloat reads any number a wire or code value may hold.
func toFloat(v any) (float64, bool) {
	switch n := v.(type) {
	case json.Number:
		f, err := strconv.ParseFloat(string(n), 64)
		return f, err == nil
	case float64:
		return n, true
	case float32:
		return float64(n), true
	case int:
		return float64(n), true
	case int32:
		return float64(n), true
	case int64:
		return float64(n), true
	}
	return 0, false
}

var intRange = map[string][2]*big.Int{
	"int32": {big.NewInt(math.MinInt32), big.NewInt(math.MaxInt32)},
	"int":   {big.NewInt(math.MinInt64), big.NewInt(math.MaxInt64)},
	"int64": {big.NewInt(math.MinInt64), big.NewInt(math.MaxInt64)},
}

func (w *walker) integer(name string, value any, path string) any {
	var n *big.Int
	switch v := value.(type) {
	case int:
		n = big.NewInt(int64(v))
	case int32:
		n = big.NewInt(int64(v))
	case int64:
		n = big.NewInt(v)
	case *big.Int:
		n = v
	case json.Number, float64:
		f, ok := toFloat(v)
		if !ok || math.IsInf(f, 0) || f != math.Trunc(f) || math.Abs(f) > maxSafe {
			if w.m == decode {
				w.fail(path, "must be a whole number up to 2^53 - 1 (larger values are sent as strings)")
			} else {
				w.fail(path, "must be a whole number up to 2^53 - 1 (use an int64 for larger values)")
			}
			return nil
		}
		n = big.NewInt(int64(f))
	case string:
		if w.m != decode || !wholeRe.MatchString(v) {
			w.fail(path, "must be a whole number, not %s", describe(value))
			return nil
		}
		n, _ = new(big.Int).SetString(v, 10)
	default:
		w.fail(path, "must be a whole number, not %s", describe(value))
		return nil
	}
	r := intRange[name]
	if n.Cmp(r[0]) < 0 || n.Cmp(r[1]) > 0 {
		w.fail(path, "must be between %s and %s", r[0], r[1])
		return nil
	}
	out := n.Int64()
	if w.m == encode && (out > maxSafe || out < -maxSafe) {
		return strconv.FormatInt(out, 10)
	}
	return out
}

func (w *walker) timestamp(value any, path string) any {
	if w.m == encode {
		t, ok := value.(time.Time)
		if !ok {
			w.fail(path, "must be a time.Time, not %s", describe(value))
			return nil
		}
		return t.UTC().Format("2006-01-02T15:04:05.000Z")
	}
	s, _ := value.(string)
	m := timestampRe.FindStringSubmatch(s)
	if m == nil {
		w.fail(path, "must be an RFC 3339 timestamp such as 2026-11-23T10:00:00Z")
		return nil
	}
	num := func(i int) int { n, _ := strconv.Atoi(m[i]); return n }
	year, month, day, hour, minute, second := num(1), num(2), num(3), num(4), num(5), num(6)
	if !isRealDate(year, month, day) || hour > 23 || minute > 59 || second > 59 {
		w.fail(path, "must be a real date and time")
		return nil
	}
	// Milliseconds: the fraction cut to three digits, never rounded.
	frac := strings.TrimPrefix(m[7], ".")
	for len(frac) < 3 {
		frac += "0"
	}
	ms, _ := strconv.Atoi(frac[:3])
	offset := 0
	if zone := m[8]; zone != "Z" && zone != "z" {
		h, _ := strconv.Atoi(zone[1:3])
		mi, _ := strconv.Atoi(zone[4:6])
		if h > 23 || mi > 59 {
			w.fail(path, "must be a real date and time")
			return nil
		}
		offset = h*3600 + mi*60
		if zone[0] == '-' {
			offset = -offset
		}
	}
	local := time.Date(year, time.Month(month), day, hour, minute, second, ms*int(time.Millisecond), time.UTC)
	return local.Add(-time.Duration(offset) * time.Second)
}

// ---- rules ----

var (
	patternCache sync.Map // string -> *regexp.Regexp
)

func pattern(source string) (*regexp.Regexp, error) {
	if re, ok := patternCache.Load(source); ok {
		return re.(*regexp.Regexp), nil
	}
	re, err := regexp.Compile(source)
	if err != nil {
		return nil, err
	}
	patternCache.Store(source, re)
	return re, nil
}

func (w *walker) rules(rules []Rule, value any, path string) {
	for _, r := range rules {
		switch r.Rule {
		case "min", "max":
			var n float64
			switch v := value.(type) {
			case int64:
				n = float64(v)
			case int:
				n = float64(v)
			case int32:
				n = float64(v)
			case float64:
				n = v
			case string:
				f, err := strconv.ParseFloat(v, 64)
				if err != nil {
					continue
				}
				n = f
			default:
				continue
			}
			limit, _ := strconv.ParseFloat(r.Value, 64)
			if r.Rule == "min" && n < limit {
				w.fail(path, "must be at least %s", r.Value)
			}
			if r.Rule == "max" && n > limit {
				w.fail(path, "must be at most %s", r.Value)
			}
		case "length":
			var size uint64
			var unit string
			switch v := value.(type) {
			case string:
				size, unit = uint64(utf8.RuneCountInString(v)), "characters"
			case []byte:
				size, unit = uint64(len(v)), "bytes"
			case []any:
				size, unit = uint64(len(v)), "items"
			case map[string]any:
				size, unit = uint64(len(v)), "entries"
			default:
				continue
			}
			if size < r.Min || size > r.Max {
				w.fail(path, "must have between %d and %d %s, not %d", r.Min, r.Max, unit, size)
			}
		case "pattern":
			if s, ok := value.(string); ok {
				re, err := pattern(r.Regex)
				if err != nil {
					w.fail(path, "the pattern %s cannot be used: %v", r.Regex, err)
				} else if !re.MatchString(s) {
					w.fail(path, "must match the pattern %s", r.Regex)
				}
			}
		case "format":
			if s, ok := value.(string); ok && !CheckFormat(r.Format, s) {
				w.fail(path, "must be a valid %s", r.Format)
			}
		}
	}
}
