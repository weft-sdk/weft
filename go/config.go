package weft

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"

	"gopkg.in/yaml.v3"
)

// Config is the part of weft.yaml the runtime reads (docs/06-cli.md).
type Config struct {
	Service string `json:"service"`
	// Transports holds the settings of each transport, such as http: {library: express, port: 3000}.
	Transports map[string]map[string]any `json:"transports"`
	// Topics maps an event name to the real topic or queue name.
	Topics map[string]string `json:"topics"`
	// Services says where other services live.
	Services  map[string]string `json:"services"`
	Telemetry map[string]string `json:"telemetry"`
}

// ConfigError means weft.yaml could not be used. Problems lists everything wrong, not only the first.
type ConfigError struct{ Problems []string }

func (e *ConfigError) Error() string {
	var b strings.Builder
	b.WriteString("Invalid Weft configuration:")
	for _, p := range e.Problems {
		b.WriteString("\n  - " + p)
	}
	return b.String()
}

var envRef = regexp.MustCompile(`\$\{([A-Za-z_][A-Za-z0-9_]*)\}`)

// ParseConfig reads weft.yaml text. ${VAR} reads env, and WEFT_<TRANSPORT>_<KEY> (such as
// WEFT_KAFKA_BROKERS), WEFT_SERVICES_<NAME>, WEFT_TELEMETRY_<KEY> and WEFT_SERVICE override the file.
// Every problem is reported at once, in a *ConfigError.
func ParseConfig(text string, env map[string]string) (*Config, error) {
	var raw any
	if err := yaml.Unmarshal([]byte(text), &raw); err != nil {
		return nil, &ConfigError{[]string{"weft.yaml is not valid YAML: " + err.Error()}}
	}
	if raw == nil {
		raw = map[string]any{}
	}
	top, ok := raw.(map[string]any)
	if !ok {
		return nil, &ConfigError{[]string{"weft.yaml must be a map at the top level"}}
	}

	var problems []string
	doc := interpolate(top, "", env, &problems).(map[string]any)

	service := ""
	if s, ok := doc["service"].(string); ok {
		service = s
	} else if doc["service"] != nil {
		problems = append(problems, "service must be a string")
	}

	section := func(name string) map[string]any {
		v := doc[name]
		if v == nil {
			return map[string]any{}
		}
		m, ok := v.(map[string]any)
		if !ok {
			problems = append(problems, name+" must be a map")
			return map[string]any{}
		}
		return m
	}
	strings_ := func(name string) map[string]string {
		out := map[string]string{}
		for _, k := range sortedKeys(section(name)) {
			if s, ok := section(name)[k].(string); ok {
				out[k] = s
			} else {
				problems = append(problems, fmt.Sprintf("%s.%s must be a string", name, k))
			}
		}
		return out
	}

	transports := map[string]map[string]any{}
	tsection := section("transports")
	for _, name := range sortedKeys(tsection) {
		switch v := tsection[name].(type) {
		case nil:
			transports[name] = map[string]any{}
		case map[string]any:
			copied := map[string]any{}
			for k, x := range v {
				copied[k] = x
			}
			transports[name] = copied
		default:
			problems = append(problems, fmt.Sprintf("transports.%s must be a map", name))
		}
	}
	topics := strings_("topics")
	services := strings_("services")
	telemetry := map[string]string{}
	tel := section("telemetry")
	for _, k := range sortedKeys(tel) {
		switch v := tel[k].(type) {
		case string:
			telemetry[k] = v
		case nil:
		default:
			problems = append(problems, fmt.Sprintf("telemetry.%s must be a string", k))
		}
	}

	// Overrides: the longest transport name that prefixes the variable wins.
	names := sortedKeys(mapOf(transports))
	sort.SliceStable(names, func(i, j int) bool { return len(names[i]) > len(names[j]) })
	for _, name := range sortedStringKeys(env) {
		text := env[name]
		if !strings.HasPrefix(name, "WEFT_") {
			continue
		}
		rest := strings.TrimPrefix(name, "WEFT_")
		switch {
		case rest == "SERVICE":
			service = text
		case strings.HasPrefix(rest, "SERVICES_"):
			services[strings.ToLower(strings.TrimPrefix(rest, "SERVICES_"))] = text
		case strings.HasPrefix(rest, "TELEMETRY_"):
			key := strings.TrimPrefix(rest, "TELEMETRY_")
			target := strings.ToLower(key)
			for k := range telemetry {
				if squash(k) == squash(key) {
					target = k
				}
			}
			telemetry[target] = text
		default:
			var transport string
			for _, t := range names {
				if strings.HasPrefix(rest, strings.ToUpper(t)+"_") {
					transport = t
					break
				}
			}
			if transport == "" {
				continue
			}
			key := rest[len(transport)+1:]
			settings := transports[transport]
			target := strings.ToLower(key)
			for _, k := range sortedKeys(settings) {
				if squash(k) == squash(key) {
					target = k
				}
			}
			settings[target] = coerce(settings[target], text)
		}
	}

	if service == "" {
		problems = append(problems, "service is required")
	}
	if len(problems) > 0 {
		return nil, &ConfigError{problems}
	}
	return &Config{Service: service, Transports: transports, Topics: topics, Services: services, Telemetry: telemetry}, nil
}

func mapOf(m map[string]map[string]any) map[string]any {
	out := make(map[string]any, len(m))
	for k, v := range m {
		out[k] = v
	}
	return out
}

func sortedKeys(m map[string]any) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}

func sortedStringKeys(m map[string]string) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}

func squash(s string) string { return strings.ReplaceAll(strings.ToLower(s), "_", "") }

// coerce turns an environment value into the type of the setting it replaces.
func coerce(existing any, text string) any {
	switch existing.(type) {
	case int, int64, float64:
		if strings.TrimSpace(text) == "" {
			return text
		}
		f, err := strconv.ParseFloat(text, 64)
		if err != nil {
			return text
		}
		if f == float64(int64(f)) {
			return int(f)
		}
		return f
	case bool:
		return text == "true" || text == "1"
	}
	return text
}

// interpolate replaces ${NAME} in every string, noting each variable that is not set.
func interpolate(v any, where string, env map[string]string, problems *[]string) any {
	switch x := v.(type) {
	case string:
		return envRef.ReplaceAllStringFunc(x, func(m string) string {
			name := envRef.FindStringSubmatch(m)[1]
			if value, ok := env[name]; ok {
				return value
			}
			*problems = append(*problems, fmt.Sprintf("the environment variable %s is not set (used by %s)", name, where))
			return ""
		})
	case []any:
		out := make([]any, len(x))
		for i, item := range x {
			out[i] = interpolate(item, fmt.Sprintf("%s[%d]", where, i), env, problems)
		}
		return out
	case map[string]any:
		out := make(map[string]any, len(x))
		for _, k := range sortedKeys(x) {
			child := k
			if where != "" {
				child = where + "." + k
			}
			out[k] = interpolate(x[k], child, env, problems)
		}
		return out
	}
	return v
}

// Environ returns the process environment as a map, for ParseConfig.
func Environ() map[string]string {
	out := map[string]string{}
	for _, kv := range os.Environ() {
		if k, v, ok := strings.Cut(kv, "="); ok {
			out[k] = v
		}
	}
	return out
}

// FindConfigFile looks for weft.yaml in dir and then each folder above it.
func FindConfigFile(dir string) (string, bool) {
	dir, err := filepath.Abs(dir)
	if err != nil {
		return "", false
	}
	for {
		candidate := filepath.Join(dir, "weft.yaml")
		if _, err := os.Stat(candidate); err == nil {
			return candidate, true
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return "", false
		}
		dir = parent
	}
}

// LoadConfig reads weft.yaml from path, or when path is empty from the working folder or a folder above it.
func LoadConfig(path string) (*Config, error) {
	if path == "" {
		found, ok := FindConfigFile(".")
		if !ok {
			return nil, &ConfigError{[]string{"no weft.yaml found in the working folder or any folder above it: run `weft init`"}}
		}
		path = found
	}
	text, err := os.ReadFile(path)
	if err != nil {
		return nil, &ConfigError{[]string{"cannot read " + path + ": " + err.Error()}}
	}
	return ParseConfig(string(text), Environ())
}

// TopicName is the real topic or queue name for an event: the topics: entry, else the event name.
func (c *Config) TopicName(event string) string {
	if t, ok := c.Topics[event]; ok {
		return t
	}
	return event
}

// ServiceURL is the base URL of another service, from services:.
func (c *Config) ServiceURL(service string) (string, bool) {
	u, ok := c.Services[service]
	return u, ok
}
