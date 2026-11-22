package weft

import (
	"fmt"
	"strconv"
	"strings"
)

// ProblemTypePrefix starts the RFC 9457 type of a built-in category or a declared error.
const ProblemTypePrefix = "urn:weft:error:"

// DeclaredError is an error declared in a contract (error OutOfStock { ... }). A handler returns it;
// the receiving side gets the same type back, with the same fields.
type DeclaredError struct {
	Package string
	Name    string
	// Fields are in code form.
	Fields  map[string]any
	Message string
}

func (e *DeclaredError) Error() string {
	if e.Message != "" {
		return e.Message
	}
	return e.Package + "." + e.Name
}

// Problem is an RFC 9457 problem details body.
type Problem struct {
	Type   string  `json:"type"`
	Title  string  `json:"title"`
	Status int     `json:"status"`
	Detail string  `json:"detail,omitempty"`
	Issues []Issue `json:"issues,omitempty"`
	// Fields are the fields of a declared error, in wire form.
	Fields any `json:"fields,omitempty"`
}

func categoryProblem(e *Error) Problem {
	detail := e.Message
	if e.Category == Internal {
		// The message of an Internal error may describe the server, so it is not sent.
		detail = "Internal server error"
	}
	return Problem{Type: ProblemTypePrefix + string(e.Category), Title: string(e.Category), Status: e.HTTPStatus(), Detail: detail, Issues: e.Issues}
}

// ToProblem turns anything a handler returned into the response it deserves. unexpected is set when
// this was not a clean, expected error: the server should log it.
func ToProblem(err error, route *RouteDef, c *Contracts) (status int, p Problem, unexpected error) {
	if we, ok := err.(*Error); ok {
		p = categoryProblem(we)
		if we.Category == Internal {
			unexpected = err
		}
		return p.Status, p, unexpected
	}
	if de, ok := err.(*DeclaredError); ok {
		declared := false
		if route != nil {
			for _, r := range route.Result.Errors {
				if r.Package == de.Package && r.Name == de.Name {
					declared = true
				}
			}
		}
		if declared {
			def, ok := c.errors[de.Package+"."+de.Name]
			if ok {
				fields, encErr := c.encodeFields(def.Fields, de.Fields)
				if encErr == nil {
					return def.Status, Problem{Type: ProblemTypePrefix + de.Package + "." + de.Name, Title: de.Name, Status: def.Status, Detail: de.Error(), Fields: fields}, nil
				}
				we := &Error{Category: Internal, Message: "A declared error had invalid fields", Cause: encErr}
				return 500, categoryProblem(we), we
			}
		}
		we := errorf(Internal, "The route does not declare %s.%s", de.Package, de.Name)
		return 500, categoryProblem(we), de
	}
	we := &Error{Category: Internal, Message: "Unexpected error", Cause: err}
	return 500, categoryProblem(we), err
}

var statusToCategory = func() map[int]Category {
	m := map[int]Category{}
	for c, info := range Categories {
		m[info.HTTP] = c
	}
	return m
}()

// FromProblem is the error a client raises for an error response: a *Error with its category, or a
// *DeclaredError with its fields. It mirrors ToProblem.
func FromProblem(status int, body map[string]any, c *Contracts) error {
	str := func(k string) string { s, _ := body[k].(string); return s }
	typ := str("type")
	detail := str("detail")
	if detail == "" {
		detail = str("title")
	}
	if detail == "" {
		detail = "HTTP " + strconv.Itoa(status)
	}
	if strings.HasPrefix(typ, ProblemTypePrefix) {
		name := strings.TrimPrefix(typ, ProblemTypePrefix)
		if _, ok := Categories[Category(name)]; ok {
			e := &Error{Category: Category(name), Message: detail}
			if raw, ok := body["issues"].([]any); ok {
				for _, r := range raw {
					if m, ok := r.(map[string]any); ok {
						p, _ := m["path"].(string)
						msg, _ := m["message"].(string)
						e.Issues = append(e.Issues, Issue{Path: p, Message: msg})
					}
				}
			}
			return e
		}
		if pkg, errName, ok := strings.Cut(name, "."); ok {
			if def, found := c.errors[pkg+"."+errName]; found {
				fields, err := c.decodeFields(def.Fields, body["fields"])
				if err != nil {
					return errorf(Internal, "The server sent %s with fields that do not match the contract", name)
				}
				return &DeclaredError{Package: pkg, Name: errName, Fields: fields, Message: detail}
			}
		}
	}
	category, ok := statusToCategory[status]
	if !ok {
		category = InvalidInput
		if status >= 500 {
			category = Internal
		}
	}
	return &Error{Category: category, Message: detail}
}

// encodeFields and decodeFields code the fields of a declared error.
func (c *Contracts) encodeFields(fields []FieldDef, value any) (map[string]any, error) {
	w := &walker{c: c, m: encode}
	out := w.fields(fields, value, "", env{})
	if len(w.issues) > 0 {
		return nil, invalidInput(w.issues)
	}
	m, _ := out.(map[string]any)
	return m, nil
}

func (c *Contracts) decodeFields(fields []FieldDef, value any) (map[string]any, error) {
	w := &walker{c: c, m: decode}
	out := w.fields(fields, value, "", env{})
	if len(w.issues) > 0 {
		return nil, invalidInput(w.issues)
	}
	m, _ := out.(map[string]any)
	return m, nil
}

// ---- text values: query strings, path parameters and headers ----

// coerceText turns the text of a query string, a path parameter or a header into the wire form its
// type expects.
func coerceText(t TypeRef, v any) any {
	switch x := v.(type) {
	case []string:
		out := make([]any, len(x))
		for i, s := range x {
			out[i] = coerceText(elementOf(t), s)
		}
		return out
	case string:
		if t.Kind == "list" {
			return []any{coerceText(*t.Item, x)}
		}
		if t.Kind != "primitive" {
			return x
		}
		switch t.Name {
		case "bool":
			if x == "true" {
				return true
			}
			if x == "false" {
				return false
			}
		case "float", "float32", "float64":
			if strings.TrimSpace(x) != "" {
				if f, err := strconv.ParseFloat(x, 64); err == nil {
					return f
				}
			}
		}
		return x
	}
	return v
}

func elementOf(t TypeRef) TypeRef {
	if t.Kind == "list" {
		return *t.Item
	}
	return t
}

// decodeTextRecord decodes a record whose values came from text. lookup finds a field's raw value by
// its wire name. Issues carry prefix in front of their paths.
func (c *Contracts) decodeTextRecord(t TypeRef, lookup func(wireName string) (any, bool), prefix string) (map[string]any, []Issue) {
	if t.Kind != "named" {
		return nil, nil
	}
	def, ok := c.types[t.Package+"."+t.Name]
	if !ok || def.Kind != "record" {
		return nil, nil
	}
	wire := map[string]any{}
	for _, f := range def.Fields {
		if raw, ok := lookup(f.WireName); ok {
			wire[f.WireName] = coerceText(f.Type, raw)
		}
	}
	value, err := c.Decode(t, wire)
	if err != nil {
		issues := AsError(err).Issues
		out := make([]Issue, len(issues))
		for i, is := range issues {
			path := prefix
			if is.Path != "" {
				path = prefix + "." + is.Path
			}
			out[i] = Issue{Path: path, Message: is.Message}
		}
		return nil, out
	}
	m, _ := value.(map[string]any)
	return m, nil
}

// encodeTextRecord is the inverse, for a client: a record in code form to the text values of a query
// string or headers.
func (c *Contracts) encodeTextRecord(t TypeRef, value any) (map[string][]string, error) {
	wire, err := c.Encode(t, value)
	if err != nil {
		return nil, err
	}
	out := map[string][]string{}
	for k, v := range wire.(map[string]any) {
		if v == nil {
			continue
		}
		var items []any
		if list, ok := v.([]any); ok {
			items = list
		} else {
			items = []any{v}
		}
		for _, item := range items {
			if s, ok := item.(string); ok {
				out[k] = append(out[k], s)
			} else {
				out[k] = append(out[k], fmt.Sprint(item))
			}
		}
	}
	return out, nil
}

// DecodeErrorFields decodes the fields of a declared error from wire form.
func (c *Contracts) DecodeErrorFields(pkg, name string, wire any) (map[string]any, error) {
	def, ok := c.errors[pkg+"."+name]
	if !ok {
		return nil, errorf(Internal, "The contracts have no error %s.%s", pkg, name)
	}
	return c.decodeFields(def.Fields, wire)
}

// EncodeErrorFields encodes the fields of a declared error to wire form.
func (c *Contracts) EncodeErrorFields(pkg, name string, fields map[string]any) (map[string]any, error) {
	def, ok := c.errors[pkg+"."+name]
	if !ok {
		return nil, errorf(Internal, "The contracts have no error %s.%s", pkg, name)
	}
	return c.encodeFields(def.Fields, fields)
}

// Route finds a route by its operation name, such as createOrder.
func (c *Contracts) Route(name, pkg string) (*Ir, *RouteDef, error) {
	var found []*RouteDef
	var in []*Ir
	for i := range c.packages {
		if pkg != "" && c.packages[i].Package != pkg {
			continue
		}
		for j := range c.packages[i].Routes {
			if c.packages[i].Routes[j].Name == name {
				found = append(found, &c.packages[i].Routes[j])
				in = append(in, &c.packages[i])
			}
		}
	}
	switch len(found) {
	case 0:
		return nil, nil, errorf(Internal, "The contracts have no route %s", name)
	case 1:
		return in[0], found[0], nil
	}
	return nil, nil, errorf(Internal, "The route %s is in more than one package: pass the package", name)
}
