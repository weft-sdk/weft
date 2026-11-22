package weft

import (
	"context"
	"reflect"
)

// The helpers the code that `weft gen` writes is built on. They turn the dynamic runtime into typed
// calls: a struct in, a struct out.

// ToCode turns a Go value into the code form of the type ref (see Contracts), without validating it
// beyond what the conversion needs. Marshal does both.
func (c *Contracts) ToCode(ref TypeRef, v any) (any, error) {
	return c.toCode(ref, reflect.ValueOf(v), "")
}

// FromCode stores a code-form value in out, which must be a non-nil pointer.
func (c *Contracts) FromCode(ref TypeRef, code any, out any) error {
	rv := reflect.ValueOf(out)
	if rv.Kind() != reflect.Pointer || rv.IsNil() {
		return errorf(Internal, "FromCode needs a non-nil pointer, not %T", out)
	}
	return c.fromCode(ref, code, rv.Elem(), "")
}

// PublishTyped publishes an event whose payload is a Go struct.
func PublishTyped[T any](ctx context.Context, m *Messaging, event string, payload T, o PublishOptions) error {
	_, def, err := m.weft.Contracts.Event(event, o.Package)
	if err != nil {
		return err
	}
	code, err := m.weft.Contracts.ToCode(def.Payload, payload)
	if err != nil {
		return err
	}
	return m.Publish(ctx, event, code, o)
}

// OnTyped receives an event and hands the handler its payload as a Go struct.
func OnTyped[T any](m *Messaging, event string, handler func(ctx context.Context, payload T, mc *MessageContext) error, o SubscribeOptions) error {
	_, def, err := m.weft.Contracts.Event(event, o.Package)
	if err != nil {
		return err
	}
	return m.On(event, func(ctx context.Context, code any, mc *MessageContext) error {
		var payload T
		if err := m.weft.Contracts.FromCode(def.Payload, code, &payload); err != nil {
			return err
		}
		return handler(ctx, payload, mc)
	}, o)
}

// NewDeclaredError makes the error a handler returns for an error declared in the contracts, from a
// struct holding its fields.
func (c *Contracts) NewDeclaredError(pkg, name string, fields any, message string) *DeclaredError {
	def, ok := c.errors[pkg+"."+name]
	if !ok {
		return &DeclaredError{Package: pkg, Name: name, Message: message}
	}
	code, err := c.recordToCode(def.Fields, indirect(reflect.ValueOf(fields)), "", env{})
	if err != nil {
		return &DeclaredError{Package: pkg, Name: name, Message: message}
	}
	m, _ := code.(map[string]any)
	return &DeclaredError{Package: pkg, Name: name, Fields: m, Message: message}
}

// DeclaredFields reads the fields of a declared error into T, if err is that error.
func DeclaredFields[T any](c *Contracts, err error, pkg, name string) (T, bool) {
	var out T
	de, ok := err.(*DeclaredError)
	if !ok || de.Package != pkg || de.Name != name {
		return out, false
	}
	def, found := c.errors[pkg+"."+name]
	if !found {
		return out, false
	}
	w := &walker{c: c}
	_ = w
	rv := reflect.ValueOf(&out).Elem()
	if rv.Kind() != reflect.Struct {
		return out, false
	}
	byWire := structFields(rv.Type())
	for _, f := range def.Fields {
		idx, ok := byWire[f.WireName]
		if !ok {
			return out, false
		}
		if v, present := de.Fields[f.Name]; present && v != nil {
			if err := c.fromCode(f.Type, v, rv.Field(idx), f.Name); err != nil {
				return out, false
			}
		}
	}
	return out, true
}
