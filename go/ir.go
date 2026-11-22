package weft

import "encoding/json"

// The checked contracts (the IR), as Go types. They mirror spec/ir.md. Documents come from
// the generated code or from json.Unmarshal.

// TypeRef refers to a type. Kind is "primitive", "named", "param", "list" or "map".
type TypeRef struct {
	Kind    string    `json:"kind"`
	Name    string    `json:"name,omitempty"`    // primitive, named, param
	Package string    `json:"package,omitempty"` // named
	Args    []TypeRef `json:"args,omitempty"`    // named
	Item    *TypeRef  `json:"item,omitempty"`    // list
	Key     *TypeRef  `json:"key,omitempty"`     // map
	Value   *TypeRef  `json:"value,omitempty"`   // map
}

// Rule is one validation rule on a field. Rule is "min", "max", "length", "pattern" or "format".
type Rule struct {
	Rule   string `json:"rule"`
	Value  string `json:"value,omitempty"`  // min, max: a number as text
	Min    uint64 `json:"min,omitempty"`    // length
	Max    uint64 `json:"max,omitempty"`    // length
	Regex  string `json:"regex,omitempty"`  // pattern
	Format string `json:"format,omitempty"` // format
}

type FieldDef struct {
	Name     string   `json:"name"`
	WireName string   `json:"wireName"`
	Type     TypeRef  `json:"type"`
	Optional bool     `json:"optional,omitempty"`
	Rules    []Rule   `json:"rules,omitempty"`
	Doc      []string `json:"doc,omitempty"`
}

type VariantDef struct {
	Name string `json:"name"`
}

// TypeDef is a record (Fields) or an enum (Variants), told apart by Kind.
type TypeDef struct {
	Name     string       `json:"name"`
	Kind     string       `json:"kind"` // "record" or "enum"
	Params   []string     `json:"params,omitempty"`
	Inline   bool         `json:"inline,omitempty"`
	Fields   []FieldDef   `json:"fields,omitempty"`
	Variants []VariantDef `json:"variants,omitempty"`
}

type ErrorDef struct {
	Name   string     `json:"name"`
	Status int        `json:"status"`
	Fields []FieldDef `json:"fields"`
}

type ErrorRef struct {
	Package string `json:"package"`
	Name    string `json:"name"`
}

type ResultDef struct {
	Value    TypeRef    `json:"value"`
	Optional bool       `json:"optional,omitempty"`
	Stream   bool       `json:"stream,omitempty"`
	Errors   []ErrorRef `json:"errors,omitempty"`
}

type ParamDef struct {
	Name     string  `json:"name"`
	Type     TypeRef `json:"type"`
	Optional bool    `json:"optional,omitempty"`
}

type EventDef struct {
	Name    string   `json:"name"`
	Payload TypeRef  `json:"payload"`
	Key     []string `json:"key,omitempty"`
	Headers *TypeRef `json:"headers,omitempty"`
}

type RouteDef struct {
	Name       string     `json:"name"`
	Method     string     `json:"method"`
	Base       string     `json:"base,omitempty"`
	Path       string     `json:"path"`
	PathParams []ParamDef `json:"pathParams,omitempty"`
	Query      *TypeRef   `json:"query,omitempty"`
	Body       *TypeRef   `json:"body,omitempty"`
	Headers    *TypeRef   `json:"headers,omitempty"`
	Result     ResultDef  `json:"result"`
}

// Ir is one package's contracts. Operations other than events and routes are read by later tasks.
type Ir struct {
	IrVersion string     `json:"irVersion"`
	Package   string     `json:"package"`
	Version   string     `json:"version,omitempty"`
	Imports   []string   `json:"imports,omitempty"`
	Types     []TypeDef  `json:"types,omitempty"`
	Errors    []ErrorDef `json:"errors,omitempty"`
	Events    []EventDef `json:"events,omitempty"`
	Routes    []RouteDef `json:"routes,omitempty"`

	// Not read yet.
	RpcServices json.RawMessage `json:"rpcServices,omitempty"`
	Graphql     json.RawMessage `json:"graphql,omitempty"`
	Sockets     json.RawMessage `json:"sockets,omitempty"`
}

// Primitive returns a reference to a primitive type: Primitive("int64").
func Primitive(name string) TypeRef { return TypeRef{Kind: "primitive", Name: name} }

// Named returns a reference to a named type, with type arguments if it is generic.
func Named(pkg, name string, args ...TypeRef) TypeRef {
	return TypeRef{Kind: "named", Package: pkg, Name: name, Args: args}
}

// List returns a reference to a list of item.
func List(item TypeRef) TypeRef { return TypeRef{Kind: "list", Item: &item} }

// Map returns a reference to a map from key to value.
func Map(key, value TypeRef) TypeRef { return TypeRef{Kind: "map", Key: &key, Value: &value} }
