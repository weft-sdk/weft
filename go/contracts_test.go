package weft_test

import (
	"encoding/json"
	"errors"
	"math"
	"sync"
	"testing"
	"time"

	weft "github.com/weft-sdk/weft/go"
)

const id = "123e4567-e89b-42d3-a456-426614174000"

func field(name string, t weft.TypeRef, rules ...weft.Rule) weft.FieldDef {
	return weft.FieldDef{Name: name, WireName: name, Type: t, Rules: rules}
}

func optional(f weft.FieldDef) weft.FieldDef {
	f.Optional = true
	return f
}

var contracts = weft.NewContracts(weft.Ir{
	IrVersion: "0.1",
	Package:   "t",
	Types: []weft.TypeDef{
		{Name: "Currency", Kind: "enum", Variants: []weft.VariantDef{{Name: "EUR"}, {Name: "GBP"}}},
		{Name: "Money", Kind: "record", Fields: []weft.FieldDef{
			field("amount", weft.Primitive("int")),
			field("currency", weft.Named("t", "Currency")),
		}},
		{Name: "Order", Kind: "record", Fields: []weft.FieldDef{
			field("id", weft.Primitive("uuid")),
			{Name: "customerId", WireName: "customer_id", Type: weft.Primitive("string")},
			field("total", weft.Named("t", "Money")),
			optional(field("note", weft.Primitive("string"))),
			field("placedAt", weft.Primitive("timestamp")),
			field("blob", weft.Primitive("bytes"), weft.Rule{Rule: "length", Min: 2, Max: 4}),
			field("big", weft.Primitive("int64")),
		}},
		{Name: "Page", Kind: "record", Params: []string{"T"}, Fields: []weft.FieldDef{
			field("items", weft.List(weft.TypeRef{Kind: "param", Name: "T"})),
		}},
	},
})

var order = weft.Named("t", "Order")

func wire(t *testing.T, text string) any {
	t.Helper()
	v, err := weft.ParseJSON([]byte(text))
	if err != nil {
		t.Fatal(err)
	}
	return v
}

func issuePaths(err error) []string {
	var paths []string
	for _, i := range weft.AsError(err).Issues {
		paths = append(paths, i.Path)
	}
	return paths
}

func TestDecodeThenEncodeIsTheCanonicalWireForm(t *testing.T) {
	in := wire(t, `{"id":"`+id+`","customer_id":"c1","total":{"amount":450,"currency":"EUR"},"note":null,"placedAt":"2026-11-23T10:00:00+02:00","blob":"AQID","big":"9007199254740993","extra":1}`)
	value, err := contracts.Decode(order, in)
	if err != nil {
		t.Fatal(err)
	}
	rec := value.(map[string]any)
	if rec["customerId"] != "c1" {
		t.Errorf("the record is keyed by field names: %v", rec)
	}
	if got := rec["placedAt"].(time.Time); !got.Equal(time.Date(2026, 11, 23, 8, 0, 0, 0, time.UTC)) {
		t.Errorf("placedAt = %v", got)
	}
	if got := rec["blob"].([]byte); len(got) != 3 {
		t.Errorf("blob = %v", got)
	}
	if rec["big"].(int64) != 9007199254740993 {
		t.Errorf("big = %v", rec["big"])
	}
	if v, ok := rec["note"]; !ok || v != nil {
		t.Errorf("a null optional is kept as nil in code form: %v", rec)
	}

	out, err := contracts.Encode(order, value)
	if err != nil {
		t.Fatal(err)
	}
	text, _ := json.Marshal(out)
	var back map[string]any
	_ = json.Unmarshal(text, &back)
	if back["customer_id"] != "c1" || back["placedAt"] != "2026-11-23T08:00:00.000Z" || back["big"] != "9007199254740993" {
		t.Errorf("encoded = %s", text)
	}
	if _, ok := back["note"]; ok {
		t.Errorf("a missing optional is left out on the wire: %s", text)
	}
	if _, ok := back["extra"]; ok {
		t.Errorf("a field the contract does not have is not passed on: %s", text)
	}
}

func TestEveryProblemIsReportedWithItsPath(t *testing.T) {
	_, err := contracts.Decode(order, wire(t, `{"id":"x","total":{"amount":1.5,"currency":"JPY"},"placedAt":"soon","blob":"AQ==","big":true}`))
	e := weft.AsError(err)
	if e.Category != weft.InvalidInput {
		t.Fatalf("category = %s", e.Category)
	}
	want := map[string]bool{"id": true, "customer_id": true, "total.amount": true, "total.currency": true, "placedAt": true, "blob": true, "big": true}
	got := map[string]bool{}
	for _, p := range issuePaths(err) {
		got[p] = true
	}
	for p := range want {
		if !got[p] {
			t.Errorf("missing an issue at %s (got %v)", p, issuePaths(err))
		}
	}
	if len(got) != len(want) {
		t.Errorf("got %v", issuePaths(err))
	}
	if e.Message == "" || e.HTTPStatus() != 400 || e.GRPCCode() != "INVALID_ARGUMENT" {
		t.Errorf("error = %+v", e)
	}
}

func TestEncodeValidatesTheCodeForm(t *testing.T) {
	good := map[string]any{
		"id": id, "customerId": "c", "total": map[string]any{"amount": int64(5), "currency": "GBP"},
		"placedAt": time.Date(2026, 1, 2, 3, 4, 5, 6_000_000, time.FixedZone("x", 3600)),
		"blob":     []byte{1, 2, 3}, "big": int64(math.MaxInt64),
	}
	out, err := contracts.Encode(order, good)
	if err != nil {
		t.Fatal(err)
	}
	rec := out.(map[string]any)
	if rec["placedAt"] != "2026-01-02T02:04:05.006Z" || rec["big"] != "9223372036854775807" || rec["blob"] != "AQID" {
		t.Errorf("encoded = %v", rec)
	}

	// Rules read the code form: three bytes are fine though their base64 is four characters, and one is too few.
	short := map[string]any{}
	for k, v := range good {
		short[k] = v
	}
	short["blob"] = []byte{1}
	if _, err := contracts.Encode(order, short); err == nil {
		t.Error("one byte should break @length(2, 4)")
	}

	bad := map[string]any{"id": 5, "customerId": "c", "total": "x", "placedAt": "not a time", "blob": "AQID", "big": 1.5}
	if issues := contracts.Validate(order, bad); len(issues) != 5 {
		t.Errorf("issues = %v", issues)
	}
	if issues := contracts.Validate(order, good); len(issues) != 0 {
		t.Errorf("issues = %v", issues)
	}
}

func TestIntegersAreExact(t *testing.T) {
	cases := []struct {
		name string
		wire string
		ok   bool
		want int64
	}{
		{"int", "5", true, 5},
		{"int", `"5"`, true, 5},
		{"int", "2.0", true, 2},
		{"int", "1e2", true, 100},
		{"int", "1.5", false, 0},
		{"int", "9007199254740992", false, 0},
		{"int", `"9007199254740993"`, true, 9007199254740993},
		{"int", `"9223372036854775808"`, false, 0},
		{"int32", "2147483647", true, 2147483647},
		{"int32", "2147483648", false, 0},
	}
	for _, c := range cases {
		v, err := contracts.Decode(weft.Primitive(c.name), wire(t, c.wire))
		if c.ok != (err == nil) || (c.ok && v.(int64) != c.want) {
			t.Errorf("%s %s: got %v, %v", c.name, c.wire, v, err)
		}
	}
}

func TestGenericsMapsAndListsCarryTheirPaths(t *testing.T) {
	page := weft.Named("t", "Page", weft.Named("t", "Money"))
	_, err := contracts.Decode(page, wire(t, `{"items":[{"amount":1,"currency":"EUR"},{"amount":1,"currency":"JPY"}]}`))
	if p := issuePaths(err); len(p) != 1 || p[0] != "items[1].currency" {
		t.Errorf("paths = %v", p)
	}
	m := weft.Map(weft.Named("t", "Currency"), weft.Primitive("int"))
	if _, err := contracts.Decode(m, wire(t, `{"EUR":1}`)); err != nil {
		t.Error(err)
	}
	if p := issuePaths(errOf(contracts.Decode(m, wire(t, `{"USD":1}`)))); len(p) != 1 || p[0] != "[USD]" {
		t.Errorf("paths = %v", p)
	}
}

func errOf(_ any, err error) error { return err }

func TestJSONHelpers(t *testing.T) {
	if _, err := contracts.DecodeJSON(order, []byte(`{nope`)); weft.AsError(err).Category != weft.InvalidInput {
		t.Errorf("err = %v", err)
	}
	if _, err := weft.ParseJSON([]byte(`1 2`)); err == nil {
		t.Error("two JSON values are not one")
	}
	text, err := contracts.EncodeJSON(weft.Primitive("int"), int64(1)<<60)
	if err != nil || string(text) != `"1152921504606846976"` {
		t.Errorf("EncodeJSON = %s, %v", text, err)
	}
}

func TestAContractsCanBeUsedFromManyGoroutines(t *testing.T) {
	in := wire(t, `{"id":"`+id+`","customer_id":"c1","total":{"amount":450,"currency":"EUR"},"placedAt":"2026-11-23T10:00:00Z","blob":"AQID","big":1}`)
	var wg sync.WaitGroup
	for i := 0; i < 32; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := 0; j < 50; j++ {
				if v, err := contracts.Decode(order, in); err != nil {
					t.Error(err)
				} else if _, err := contracts.Encode(order, v); err != nil {
					t.Error(err)
				}
			}
		}()
	}
	wg.Wait()
}

func TestErrorCategoriesMatchTheSpec(t *testing.T) {
	table := []struct {
		c    weft.Category
		http int
		grpc string
		msg  weft.MessagingPolicy
	}{
		{weft.NotFound, 404, "NOT_FOUND", weft.PolicyNone},
		{weft.InvalidInput, 400, "INVALID_ARGUMENT", weft.PolicyDeadLetter},
		{weft.Unauthorized, 401, "UNAUTHENTICATED", weft.PolicyNone},
		{weft.Forbidden, 403, "PERMISSION_DENIED", weft.PolicyNone},
		{weft.Conflict, 409, "ALREADY_EXISTS", weft.PolicyNone},
		{weft.Unavailable, 503, "UNAVAILABLE", weft.PolicyRetry},
		{weft.Timeout, 504, "DEADLINE_EXCEEDED", weft.PolicyRetry},
		{weft.Internal, 500, "INTERNAL", weft.PolicyRetryThenDeadLetter},
	}
	if len(weft.Categories) != len(table) {
		t.Fatalf("%d categories", len(weft.Categories))
	}
	for _, r := range table {
		e := weft.NewError(r.c, "x")
		if e.HTTPStatus() != r.http || e.GRPCCode() != r.grpc || e.MessagingPolicy() != r.msg {
			t.Errorf("%s: %d %s %s", r.c, e.HTTPStatus(), e.GRPCCode(), e.MessagingPolicy())
		}
	}
}

func TestAsErrorFindsOrWraps(t *testing.T) {
	inner := weft.NewError(weft.NotFound, "gone")
	wrapped := errors.Join(errors.New("context"), inner)
	if weft.AsError(wrapped) != inner {
		t.Error("an *Error in the chain is returned as it is")
	}
	plain := errors.New("boom")
	got := weft.AsError(plain)
	if got.Category != weft.Internal || !errors.Is(got, plain) {
		t.Errorf("got %+v", got)
	}
}
