package weft

import (
	"fmt"
	"reflect"
	"strconv"
	"strings"
	"time"
)

// Marshal turns a Go value (a struct that weft gen wrote, or any value shaped like one) into the wire
// form of the type ref, validating it on the way. Struct fields are matched to the contract's fields
// by the wire name in their `json` tag.
func (c *Contracts) Marshal(ref TypeRef, v any) (any, error) {
	code, err := c.toCode(ref, reflect.ValueOf(v), "")
	if err != nil {
		return nil, err
	}
	return c.Encode(ref, code)
}

// Unmarshal decodes a wire value as the type ref and stores it in out, which must be a non-nil pointer.
func (c *Contracts) Unmarshal(ref TypeRef, wire any, out any) error {
	rv := reflect.ValueOf(out)
	if rv.Kind() != reflect.Pointer || rv.IsNil() {
		return errorf(Internal, "Unmarshal needs a non-nil pointer, not %T", out)
	}
	code, err := c.Decode(ref, wire)
	if err != nil {
		return err
	}
	return c.fromCode(ref, code, rv.Elem(), "")
}

// ---- Go value to code form ----

func indirect(rv reflect.Value) reflect.Value {
	for rv.IsValid() && (rv.Kind() == reflect.Pointer || rv.Kind() == reflect.Interface) {
		if rv.IsNil() {
			return reflect.Value{}
		}
		rv = rv.Elem()
	}
	return rv
}

func (c *Contracts) toCode(ref TypeRef, rv reflect.Value, path string) (any, error) {
	return c.toCodeIn(ref, rv, path, nil)
}

func (c *Contracts) toCodeIn(ref TypeRef, rv reflect.Value, path string, e env) (any, error) {
	rv = indirect(rv)
	if !rv.IsValid() {
		return nil, nil
	}
	switch ref.Kind {
	case "primitive":
		return primitiveToCode(ref.Name, rv, path)
	case "param":
		bound, ok := e[ref.Name]
		if !ok {
			return nil, errorf(Internal, "the type parameter %s has no argument", ref.Name)
		}
		return c.toCodeIn(bound, rv, path, nil)
	case "list":
		if rv.Kind() != reflect.Slice && rv.Kind() != reflect.Array {
			return nil, errorf(InvalidInput, "%s: expected a slice, not %s", showPath(path), rv.Type())
		}
		out := make([]any, rv.Len())
		for i := range out {
			item, err := c.toCodeIn(*ref.Item, rv.Index(i), fmt.Sprintf("%s[%d]", path, i), e)
			if err != nil {
				return nil, err
			}
			out[i] = item
		}
		return out, nil
	case "map":
		if rv.Kind() != reflect.Map {
			return nil, errorf(InvalidInput, "%s: expected a map, not %s", showPath(path), rv.Type())
		}
		out := make(map[string]any, rv.Len())
		iter := rv.MapRange()
		for iter.Next() {
			key := fmt.Sprint(indirect(iter.Key()).Interface())
			val, err := c.toCodeIn(*ref.Value, iter.Value(), fmt.Sprintf("%s[%s]", path, key), e)
			if err != nil {
				return nil, err
			}
			out[key] = val
		}
		return out, nil
	case "named":
		def, ok := c.types[ref.Package+"."+ref.Name]
		if !ok {
			return nil, errorf(Internal, "the contracts have no type %s.%s", ref.Package, ref.Name)
		}
		if def.Kind == "enum" {
			if rv.Kind() != reflect.String {
				return nil, errorf(InvalidInput, "%s: an enum is a string, not %s", showPath(path), rv.Type())
			}
			return rv.String(), nil
		}
		inner := env{}
		w := &walker{c: c}
		for i, p := range def.Params {
			if i < len(ref.Args) {
				inner[p] = w.subst(ref.Args[i], e)
			}
		}
		return c.recordToCode(def.Fields, rv, path, inner)
	}
	return nil, errorf(Internal, "unknown type kind %q", ref.Kind)
}

func (c *Contracts) recordToCode(fields []FieldDef, rv reflect.Value, path string, e env) (any, error) {
	if rv.Kind() != reflect.Struct {
		return nil, errorf(InvalidInput, "%s: expected a struct, not %s", showPath(path), rv.Type())
	}
	byWire := structFields(rv.Type())
	out := make(map[string]any, len(fields))
	w := &walker{c: c}
	for _, f := range fields {
		idx, ok := byWire[f.WireName]
		if !ok {
			return nil, errorf(Internal, "%s has no field for %q: add `json:%q`", rv.Type(), f.WireName, f.WireName)
		}
		fv := rv.Field(idx)
		if indirect(fv).IsValid() {
			code, err := c.toCodeIn(w.subst(f.Type, e), fv, join(path, f.Name), e)
			if err != nil {
				return nil, err
			}
			out[f.Name] = code
		}
	}
	return out, nil
}

// structFields maps each json wire name of a struct to the index of its field.
func structFields(t reflect.Type) map[string]int {
	out := map[string]int{}
	for i := 0; i < t.NumField(); i++ {
		sf := t.Field(i)
		if !sf.IsExported() {
			continue
		}
		name, _, _ := strings.Cut(sf.Tag.Get("json"), ",")
		if name == "" || name == "-" {
			name = sf.Name
		}
		out[name] = i
	}
	return out
}

func showPath(p string) string {
	if p == "" {
		return "the value"
	}
	return p
}

func primitiveToCode(name string, rv reflect.Value, path string) (any, error) {
	bad := func() (any, error) {
		return nil, errorf(InvalidInput, "%s: a %s cannot be held in %s", showPath(path), name, rv.Type())
	}
	switch name {
	case "string", "decimal", "uuid":
		if rv.Kind() == reflect.String {
			return rv.String(), nil
		}
	case "bool":
		if rv.Kind() == reflect.Bool {
			return rv.Bool(), nil
		}
	case "int", "int32", "int64":
		switch rv.Kind() {
		case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
			return rv.Int(), nil
		case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32:
			return int64(rv.Uint()), nil
		}
	case "float", "float32", "float64":
		if rv.Kind() == reflect.Float32 || rv.Kind() == reflect.Float64 {
			return rv.Float(), nil
		}
	case "timestamp":
		if t, ok := rv.Interface().(time.Time); ok {
			return t, nil
		}
	case "bytes":
		if rv.Kind() == reflect.Slice && rv.Type().Elem().Kind() == reflect.Uint8 {
			return rv.Bytes(), nil
		}
	}
	return bad()
}

// ---- code form to Go value ----

func (c *Contracts) fromCode(ref TypeRef, code any, out reflect.Value, path string) error {
	return c.fromCodeIn(ref, code, out, path, nil)
}

func (c *Contracts) fromCodeIn(ref TypeRef, code any, out reflect.Value, path string, e env) error {
	if code == nil {
		return nil
	}
	if out.Kind() == reflect.Pointer {
		if out.IsNil() {
			out.Set(reflect.New(out.Type().Elem()))
		}
		return c.fromCodeIn(ref, code, out.Elem(), path, e)
	}
	switch ref.Kind {
	case "primitive":
		return primitiveFromCode(ref.Name, code, out, path)
	case "param":
		bound, ok := e[ref.Name]
		if !ok {
			return errorf(Internal, "the type parameter %s has no argument", ref.Name)
		}
		return c.fromCodeIn(bound, code, out, path, nil)
	case "list":
		list, _ := code.([]any)
		if out.Kind() != reflect.Slice {
			return errorf(Internal, "%s: a list cannot be held in %s", showPath(path), out.Type())
		}
		s := reflect.MakeSlice(out.Type(), len(list), len(list))
		for i, item := range list {
			if err := c.fromCodeIn(*ref.Item, item, s.Index(i), fmt.Sprintf("%s[%d]", path, i), e); err != nil {
				return err
			}
		}
		out.Set(s)
		return nil
	case "map":
		m, _ := code.(map[string]any)
		if out.Kind() != reflect.Map {
			return errorf(Internal, "%s: a map cannot be held in %s", showPath(path), out.Type())
		}
		result := reflect.MakeMapWithSize(out.Type(), len(m))
		for k, v := range m {
			key := reflect.New(out.Type().Key()).Elem()
			if err := setFromText(key, k); err != nil {
				return errorf(Internal, "%s[%s]: %v", showPath(path), k, err)
			}
			val := reflect.New(out.Type().Elem()).Elem()
			if err := c.fromCodeIn(*ref.Value, v, val, fmt.Sprintf("%s[%s]", path, k), e); err != nil {
				return err
			}
			result.SetMapIndex(key, val)
		}
		out.Set(result)
		return nil
	case "named":
		def, ok := c.types[ref.Package+"."+ref.Name]
		if !ok {
			return errorf(Internal, "the contracts have no type %s.%s", ref.Package, ref.Name)
		}
		if def.Kind == "enum" {
			if out.Kind() != reflect.String {
				return errorf(Internal, "%s: an enum cannot be held in %s", showPath(path), out.Type())
			}
			out.SetString(code.(string))
			return nil
		}
		inner := env{}
		w := &walker{c: c}
		for i, p := range def.Params {
			if i < len(ref.Args) {
				inner[p] = w.subst(ref.Args[i], e)
			}
		}
		rec, _ := code.(map[string]any)
		if out.Kind() != reflect.Struct {
			return errorf(Internal, "%s: a record cannot be held in %s", showPath(path), out.Type())
		}
		byWire := structFields(out.Type())
		for _, f := range def.Fields {
			idx, ok := byWire[f.WireName]
			if !ok {
				return errorf(Internal, "%s has no field for %q: add `json:%q`", out.Type(), f.WireName, f.WireName)
			}
			if v, present := rec[f.Name]; present && v != nil {
				if err := c.fromCodeIn(w.subst(f.Type, inner), v, out.Field(idx), join(path, f.Name), inner); err != nil {
					return err
				}
			}
		}
		return nil
	}
	return errorf(Internal, "unknown type kind %q", ref.Kind)
}

func primitiveFromCode(name string, code any, out reflect.Value, path string) error {
	bad := func() error {
		return errorf(Internal, "%s: a %s cannot be held in %s", showPath(path), name, out.Type())
	}
	switch name {
	case "string", "decimal", "uuid":
		if out.Kind() == reflect.String {
			out.SetString(code.(string))
			return nil
		}
	case "bool":
		if out.Kind() == reflect.Bool {
			out.SetBool(code.(bool))
			return nil
		}
	case "int", "int32", "int64":
		n := code.(int64)
		switch out.Kind() {
		case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
			if out.OverflowInt(n) {
				return errorf(Internal, "%s: %d does not fit in %s", showPath(path), n, out.Type())
			}
			out.SetInt(n)
			return nil
		}
	case "float", "float32", "float64":
		if out.Kind() == reflect.Float32 || out.Kind() == reflect.Float64 {
			out.SetFloat(code.(float64))
			return nil
		}
	case "timestamp":
		if _, ok := out.Interface().(time.Time); ok {
			out.Set(reflect.ValueOf(code.(time.Time)))
			return nil
		}
	case "bytes":
		if out.Kind() == reflect.Slice && out.Type().Elem().Kind() == reflect.Uint8 {
			out.SetBytes(code.([]byte))
			return nil
		}
	}
	return bad()
}

func setFromText(v reflect.Value, s string) error {
	switch v.Kind() {
	case reflect.String:
		v.SetString(s)
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		n, err := strconv.ParseInt(s, 10, 64)
		if err != nil {
			return err
		}
		v.SetInt(n)
	default:
		return fmt.Errorf("a map key of type %s is not supported", v.Type())
	}
	return nil
}
