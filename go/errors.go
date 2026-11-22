package weft

import (
	"errors"
	"fmt"
)

// Issue is one thing wrong with a value, found by validation.
type Issue struct {
	// Path says where, such as "items[0].qty". Empty for the value itself.
	Path    string `json:"path"`
	Message string `json:"message"`
}

// Category is one of the eight error categories every language shares (spec/behaviour.md).
type Category string

const (
	NotFound     Category = "NotFound"
	InvalidInput Category = "InvalidInput"
	Unauthorized Category = "Unauthorized"
	Forbidden    Category = "Forbidden"
	Conflict     Category = "Conflict"
	Unavailable  Category = "Unavailable"
	Timeout      Category = "Timeout"
	Internal     Category = "Internal"
)

// MessagingPolicy is what a messaging consumer does when a handler fails with a category.
type MessagingPolicy string

const (
	PolicyNone                MessagingPolicy = "none"
	PolicyDeadLetter          MessagingPolicy = "dead-letter"
	PolicyRetry               MessagingPolicy = "retry"
	PolicyRetryThenDeadLetter MessagingPolicy = "retry-then-dead-letter"
)

// CategoryInfo says how a category maps to each transport.
type CategoryInfo struct {
	HTTP      int
	GRPC      string
	Messaging MessagingPolicy
}

// Categories is the table in spec/behaviour.md.
var Categories = map[Category]CategoryInfo{
	NotFound:     {404, "NOT_FOUND", PolicyNone},
	InvalidInput: {400, "INVALID_ARGUMENT", PolicyDeadLetter},
	Unauthorized: {401, "UNAUTHENTICATED", PolicyNone},
	Forbidden:    {403, "PERMISSION_DENIED", PolicyNone},
	Conflict:     {409, "ALREADY_EXISTS", PolicyNone},
	Unavailable:  {503, "UNAVAILABLE", PolicyRetry},
	Timeout:      {504, "DEADLINE_EXCEEDED", PolicyRetry},
	Internal:     {500, "INTERNAL", PolicyRetryThenDeadLetter},
}

// Error is an error with one of the eight categories.
type Error struct {
	Category Category
	Message  string
	// Issues is set for InvalidInput raised by validation.
	Issues []Issue
	Cause  error
}

func (e *Error) Error() string { return e.Message }
func (e *Error) Unwrap() error { return e.Cause }

// HTTPStatus is the status of the category.
func (e *Error) HTTPStatus() int { return Categories[e.Category].HTTP }

// GRPCCode is the gRPC status code name of the category.
func (e *Error) GRPCCode() string { return Categories[e.Category].GRPC }

// MessagingPolicy is what a consumer does with this error.
func (e *Error) MessagingPolicy() MessagingPolicy { return Categories[e.Category].Messaging }

// NewError makes an error of a category.
func NewError(category Category, message string) *Error {
	return &Error{Category: category, Message: message}
}

func errorf(category Category, format string, args ...any) *Error {
	return &Error{Category: category, Message: fmt.Sprintf(format, args...)}
}

// AsError returns the *Error in err's chain, or wraps any other error as Internal.
func AsError(err error) *Error {
	var e *Error
	if errors.As(err, &e) {
		return e
	}
	return &Error{Category: Internal, Message: err.Error(), Cause: err}
}

// invalidInput is the error for a value that failed validation. It lists every issue.
func invalidInput(issues []Issue) *Error {
	message := "Invalid input"
	if len(issues) > 0 {
		first := issues[0]
		where := ""
		if first.Path != "" {
			where = first.Path + " "
		}
		more := ""
		if len(issues) > 1 {
			more = fmt.Sprintf(" (and %d more)", len(issues)-1)
		}
		message = "Invalid input: " + where + first.Message + more
	}
	return &Error{Category: InvalidInput, Message: message, Issues: issues}
}
