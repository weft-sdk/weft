package weft_test

import (
	"reflect"
	"strings"
	"testing"
	"time"

	weft "github.com/weft-sdk/weft/go"
)

type moneyT struct {
	Amount   int64  `json:"amount"`
	Currency string `json:"currency"`
}

type orderT struct {
	ID         string    `json:"id"`
	CustomerID string    `json:"customer_id"`
	Total      moneyT    `json:"total"`
	Note       *string   `json:"note,omitempty"`
	PlacedAt   time.Time `json:"placedAt"`
	Blob       []byte    `json:"blob"`
	Big        int64     `json:"big"`
}

type pageT[T any] struct {
	Items []T `json:"items"`
}

func TestMarshalAndUnmarshalAreInverses(t *testing.T) {
	note := "hi"
	in := orderT{
		ID: id, CustomerID: "c1", Total: moneyT{Amount: 450, Currency: "EUR"}, Note: &note,
		PlacedAt: time.Date(2026, 11, 23, 10, 0, 0, 0, time.FixedZone("x", 2*3600)), Blob: []byte{1, 2, 3}, Big: 9007199254740993,
	}
	wire, err := contracts.Marshal(order, in)
	if err != nil {
		t.Fatal(err)
	}
	rec := wire.(map[string]any)
	if rec["customer_id"] != "c1" || rec["placedAt"] != "2026-11-23T08:00:00.000Z" || rec["big"] != "9007199254740993" || rec["blob"] != "AQID" || rec["note"] != "hi" {
		t.Errorf("wire = %v", rec)
	}

	var out orderT
	if err := contracts.Unmarshal(order, wire, &out); err != nil {
		t.Fatal(err)
	}
	if !out.PlacedAt.Equal(in.PlacedAt) || *out.Note != "hi" || out.Big != in.Big || out.CustomerID != "c1" || !reflect.DeepEqual(out.Blob, in.Blob) || out.Total != in.Total {
		t.Errorf("out = %+v", out)
	}
}

func TestAnOptionalFieldThatIsNilIsLeftOutAndComesBackNil(t *testing.T) {
	in := orderT{ID: id, CustomerID: "c", Total: moneyT{1, "EUR"}, PlacedAt: time.Unix(0, 0).UTC(), Blob: []byte{1, 2}}
	wire, err := contracts.Marshal(order, in)
	if err != nil {
		t.Fatal(err)
	}
	if _, present := wire.(map[string]any)["note"]; present {
		t.Errorf("note should be left out: %v", wire)
	}
	var out orderT
	if err := contracts.Unmarshal(order, wire, &out); err != nil || out.Note != nil {
		t.Errorf("%v %v", out.Note, err)
	}
}

func TestMarshalValidatesWithThePathOfTheField(t *testing.T) {
	bad := orderT{ID: "nope", CustomerID: "c", Total: moneyT{1, "JPY"}, PlacedAt: time.Now(), Blob: []byte{1}}
	_, err := contracts.Marshal(order, bad)
	e := weft.AsError(err)
	if e.Category != weft.InvalidInput {
		t.Fatalf("err = %v", err)
	}
	var paths []string
	for _, i := range e.Issues {
		paths = append(paths, i.Path)
	}
	for _, want := range []string{"id", "total.currency", "blob"} {
		found := false
		for _, p := range paths {
			found = found || p == want
		}
		if !found {
			t.Errorf("no issue at %s in %v", want, paths)
		}
	}
}

func TestUnmarshalOfAWireValueThatIsInvalidFails(t *testing.T) {
	var out orderT
	err := contracts.Unmarshal(order, map[string]any{"id": "x"}, &out)
	if weft.AsError(err).Category != weft.InvalidInput {
		t.Errorf("err = %v", err)
	}
	if err := contracts.Unmarshal(order, map[string]any{}, out); weft.AsError(err).Category != weft.Internal {
		t.Errorf("a non-pointer is a programming error: %v", err)
	}
}

func TestGenericsAreFilledIn(t *testing.T) {
	ref := weft.Named("t", "Page", weft.Named("t", "Money"))
	wire, err := contracts.Marshal(ref, pageT[moneyT]{Items: []moneyT{{1, "EUR"}, {2, "GBP"}}})
	if err != nil {
		t.Fatal(err)
	}
	var out pageT[moneyT]
	if err := contracts.Unmarshal(ref, wire, &out); err != nil || len(out.Items) != 2 || out.Items[1].Currency != "GBP" {
		t.Errorf("%+v %v", out, err)
	}
	if _, err := contracts.Marshal(ref, pageT[moneyT]{Items: []moneyT{{1, "JPY"}}}); err == nil {
		t.Error("a bad item should fail")
	}
}

func TestMapsAndLists(t *testing.T) {
	ref := weft.Map(weft.Named("t", "Currency"), weft.List(weft.Primitive("int")))
	type currency string
	wire, err := contracts.Marshal(ref, map[currency][]int{"EUR": {1, 2}})
	if err != nil {
		t.Fatal(err)
	}
	var out map[currency][]int
	if err := contracts.Unmarshal(ref, wire, &out); err != nil || !reflect.DeepEqual(out, map[currency][]int{"EUR": {1, 2}}) {
		t.Errorf("%v %v", out, err)
	}
	if _, err := contracts.Marshal(ref, map[currency][]int{"USD": {1}}); err == nil {
		t.Error("USD is not a currency of the contract")
	}
	ints := weft.Map(weft.Primitive("int"), weft.Primitive("string"))
	var byInt map[int64]string
	if err := contracts.Unmarshal(ints, map[string]any{"1": "a", "-2": "b"}, &byInt); err != nil || byInt[-2] != "b" {
		t.Errorf("%v %v", byInt, err)
	}
}

func TestAStructWithoutTheFieldNamesTheMissingTag(t *testing.T) {
	type broken struct {
		Amount int64 `json:"amount"`
	}
	_, err := contracts.Marshal(weft.Named("t", "Money"), broken{Amount: 1})
	if err == nil || !strings.Contains(err.Error(), `no field for "currency"`) {
		t.Errorf("err = %v", err)
	}
}

func TestSmallIntegerFieldsAreChecked(t *testing.T) {
	type small struct {
		Amount   int8   `json:"amount"`
		Currency string `json:"currency"`
	}
	var out small
	if err := contracts.Unmarshal(weft.Named("t", "Money"), map[string]any{"amount": 1000, "currency": "EUR"}, &out); err == nil || !strings.Contains(err.Error(), "does not fit") {
		t.Errorf("err = %v", err)
	}
}
