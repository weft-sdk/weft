// Package nethttp serves Weft's HTTP routes with the standard library's net/http. net/http stays in
// charge of the socket and the router; Weft does the contracts, middleware, errors and tracing.
// Raw is the *http.ServeMux.
package nethttp

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"strconv"
	"strings"
	"sync"
	"time"

	weft "github.com/weft-sdk/weft/go"
)

// Options configure an Adapter. Zero values mean the defaults.
type Options struct {
	// Port defaults to 3000, or transports.http.port in weft.yaml. Use 0 for any free port.
	Port *int
	// Host defaults to all interfaces, or transports.http.host.
	Host string
	// Mux is your own *http.ServeMux. Without one, the adapter makes its own and answers unknown
	// paths with a 404 problem.
	Mux *http.ServeMux
	// BodyLimit is the largest request body, in bytes. Default 1 MiB.
	BodyLimit int64
	// CloseGrace is how long Close lets responses finish being written before it cuts connections. Default 2 s.
	CloseGrace time.Duration
}

// Adapter is a weft.HTTPAdapter on net/http.
type Adapter struct {
	opts     Options
	settings map[string]any
	mux      *http.ServeMux
	ownsMux  bool

	mu       sync.Mutex
	server   *http.Server
	listener net.Listener
	shutdown chan error
}

// New makes an Adapter.
func New(o Options) *Adapter {
	a := &Adapter{opts: o, mux: o.Mux, ownsMux: o.Mux == nil}
	if a.mux == nil {
		a.mux = http.NewServeMux()
	}
	return a
}

// Configure reads transports.http from weft.yaml.
func (a *Adapter) Configure(settings map[string]any) { a.settings = settings }

func (a *Adapter) Name() string { return "http" }

// Raw is the *http.ServeMux.
func (a *Adapter) Raw() any { return a.mux }

// Port is the port it is listening on, once started. Useful with port 0.
func (a *Adapter) Port() int {
	a.mu.Lock()
	defer a.mu.Unlock()
	if a.listener == nil {
		return 0
	}
	return a.listener.Addr().(*net.TCPAddr).Port
}

// Route serves a template such as /v1/orders/{id} for a method.
func (a *Adapter) Route(method, template string, handler func(weft.HTTPRequest) weft.HTTPResponse) {
	limit := a.opts.BodyLimit
	if limit == 0 {
		limit = 1 << 20
	}
	names := paramNames(template)
	a.mux.HandleFunc(method+" "+template, func(w http.ResponseWriter, r *http.Request) {
		params := make(map[string]string, len(names))
		for _, n := range names {
			params[n] = r.PathValue(n)
		}
		headers := make(map[string]string, len(r.Header))
		for k, v := range r.Header {
			headers[strings.ToLower(k)] = strings.Join(v, ", ")
		}
		var body *string
		if r.Body != nil {
			raw, err := io.ReadAll(http.MaxBytesReader(w, r.Body, limit))
			if err != nil {
				var tooBig *http.MaxBytesError
				if errors.As(err, &tooBig) {
					writeProblem(w, 413, "InvalidInput", "The request body is too large")
				} else {
					writeProblem(w, 400, "InvalidInput", "The request could not be read")
				}
				return
			}
			if len(raw) > 0 {
				s := string(raw)
				body = &s
			}
		}
		out := handler(weft.HTTPRequest{
			Method: r.Method, Template: template, Params: params, Query: r.URL.Query(), Headers: headers, Body: body, Context: r.Context(),
		})
		for k, v := range out.Headers {
			w.Header().Set(k, v)
		}
		w.WriteHeader(out.Status)
		_, _ = io.WriteString(w, out.Body)
	})
}

func paramNames(template string) []string {
	var names []string
	for _, seg := range strings.Split(template, "/") {
		if strings.HasPrefix(seg, "{") && strings.HasSuffix(seg, "}") {
			names = append(names, strings.TrimSuffix(strings.TrimPrefix(seg, "{"), "}"))
		}
	}
	return names
}

func writeProblem(w http.ResponseWriter, status int, category, detail string) {
	w.Header().Set("Content-Type", "application/problem+json")
	w.WriteHeader(status)
	fmt.Fprintf(w, `{"type":%q,"title":%q,"status":%d,"detail":%q}`, weft.ProblemTypePrefix+category, category, status, detail)
}

// Start listens and serves.
func (a *Adapter) Start(context.Context) error {
	if a.ownsMux {
		a.mux.HandleFunc("/", func(w http.ResponseWriter, _ *http.Request) { writeProblem(w, 404, "NotFound", "No such route") })
	}
	port := 3000
	switch p := a.settings["port"].(type) {
	case int:
		port = p
	case float64:
		port = int(p)
	case string:
		if n, err := strconv.Atoi(p); err == nil {
			port = n
		}
	}
	if a.opts.Port != nil {
		port = *a.opts.Port
	}
	host := a.opts.Host
	if host == "" {
		host, _ = a.settings["host"].(string)
	}
	listener, err := net.Listen("tcp", net.JoinHostPort(host, strconv.Itoa(port)))
	if err != nil {
		return err
	}
	server := &http.Server{Handler: a.mux, ReadHeaderTimeout: 10 * time.Second}
	a.mu.Lock()
	a.listener, a.server = listener, server
	a.mu.Unlock()
	go func() { _ = server.Serve(listener) }()
	return nil
}

// StopIntake stops accepting connections and asks the server to finish what it is doing. It does not wait.
func (a *Adapter) StopIntake(context.Context) error {
	a.mu.Lock()
	server := a.server
	if server == nil || a.shutdown != nil {
		a.mu.Unlock()
		return nil
	}
	a.shutdown = make(chan error, 1)
	done := a.shutdown
	a.mu.Unlock()
	go func() { done <- server.Shutdown(context.Background()) }()
	return nil
}

// Close lets responses that are being written finish, then cuts what is left after the grace.
func (a *Adapter) Close(context.Context) error {
	a.mu.Lock()
	server, done := a.server, a.shutdown
	a.mu.Unlock()
	if server == nil {
		return nil
	}
	grace := a.opts.CloseGrace
	if grace == 0 {
		grace = 2 * time.Second
	}
	if done == nil {
		return server.Close()
	}
	select {
	case <-done:
		return nil
	case <-time.After(grace):
		return server.Close()
	}
}
