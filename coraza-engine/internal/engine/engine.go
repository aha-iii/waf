package engine

import (
	"fmt"
	"strings"
	"sync"

	coreruleset "github.com/corazawaf/coraza-coreruleset/v4"
	"github.com/corazawaf/coraza/v3"
	"github.com/corazawaf/coraza/v3/types"
)

// Engine wraps an embedded Coraza WAF with OWASP CRS v4 (in-process).
type Engine struct {
	waf coraza.WAF
}

type InspectRequest struct {
	Method    string            `json:"method"`
	Host      string            `json:"host"`
	URI       string            `json:"uri"`
	Headers   map[string]string `json:"headers"`
	Body      string            `json:"body"`
	SrcIP     string            `json:"src_ip"`
	UserAgent string            `json:"user_agent"`
}

type InspectResponse struct {
	Allowed  bool   `json:"allowed"`
	Blocked  bool   `json:"blocked"`
	Action   string `json:"action"`
	RuleID   string `json:"rule_id,omitempty"`
	RuleName string `json:"rule_name,omitempty"`
	Severity string `json:"severity,omitempty"`
	Message  string `json:"message,omitempty"`
	Engine   string `json:"engine"`
	Status   int    `json:"status,omitempty"`
	Data     string `json:"data,omitempty"`
}

var (
	global     *Engine
	globalOnce sync.Once
	globalErr  error
)

// Global returns a process-wide Coraza+CRS engine (lazy init).
func Global() (*Engine, error) {
	globalOnce.Do(func() {
		global, globalErr = New()
	})
	return global, globalErr
}

// New builds Coraza with embedded CRS from coraza-coreruleset.
func New() (*Engine, error) {
	// crs-setup 示例可能含 DetectionOnly；Include 之后强制 On，保证可拦截。
	directives := strings.TrimSpace(`
SecRequestBodyAccess On
SecResponseBodyAccess Off
Include @coraza.conf-recommended
Include @crs-setup.conf.example
SecRuleEngine On
Include @owasp_crs/*.conf
`)

	cfg := coraza.NewWAFConfig().
		WithRootFS(coreruleset.FS).
		WithDirectives(directives)

	waf, err := coraza.NewWAF(cfg)
	if err != nil {
		return nil, fmt.Errorf("coraza NewWAF: %w", err)
	}
	return &Engine{waf: waf}, nil
}

// Inspect runs request phases through Coraza CRS.
func (e *Engine) Inspect(req InspectRequest) InspectResponse {
	out := InspectResponse{
		Allowed: true,
		Blocked: false,
		Action:  "allow",
		Engine:  "coraza+crs",
	}
	if e == nil || e.waf == nil {
		return out
	}

	tx := e.waf.NewTransaction()
	defer func() { _ = tx.Close() }()

	src := strings.TrimSpace(req.SrcIP)
	if src == "" {
		src = "127.0.0.1"
	}
	tx.ProcessConnection(src, 0, "127.0.0.1", 80)

	method := strings.TrimSpace(req.Method)
	if method == "" {
		method = "GET"
	}
	uri := req.URI
	if uri == "" {
		uri = "/"
	}
	tx.ProcessURI(uri, method, "HTTP/1.1")

	host := strings.TrimSpace(req.Host)
	if host != "" {
		tx.AddRequestHeader("Host", host)
	}
	ua := strings.TrimSpace(req.UserAgent)
	if ua != "" {
		tx.AddRequestHeader("User-Agent", ua)
	}
	for k, v := range req.Headers {
		if strings.EqualFold(k, "host") || strings.EqualFold(k, "user-agent") {
			continue
		}
		tx.AddRequestHeader(k, v)
	}
	if _, ok := headerLookup(req.Headers, "Content-Type"); !ok && req.Body != "" {
		tx.AddRequestHeader("Content-Type", "application/x-www-form-urlencoded")
	}

	if it := tx.ProcessRequestHeaders(); it != nil {
		return interrupted(it)
	}

	if req.Body != "" {
		if _, _, err := tx.WriteRequestBody([]byte(req.Body)); err == nil {
			if it, err := tx.ProcessRequestBody(); err == nil && it != nil {
				return interrupted(it)
			}
		}
	}

	if it := tx.Interruption(); it != nil {
		return interrupted(it)
	}
	return out
}

func headerLookup(h map[string]string, name string) (string, bool) {
	for k, v := range h {
		if strings.EqualFold(k, name) {
			return v, true
		}
	}
	return "", false
}

func interrupted(it *types.Interruption) InspectResponse {
	action := strings.ToLower(it.Action)
	if action == "" {
		action = "deny"
	}
	blocked := action == "deny" || action == "drop" || action == "reject" || action == "block"
	outAction := "block"
	if !blocked {
		outAction = action
	}
	return InspectResponse{
		Allowed:  !blocked,
		Blocked:  blocked,
		Action:   outAction,
		RuleID:   fmt.Sprintf("%d", it.RuleID),
		RuleName: fmt.Sprintf("coraza-%d", it.RuleID),
		Severity: "critical",
		Message:  it.Data,
		Engine:   "coraza+crs",
		Status:   it.Status,
		Data:     it.Data,
	}
}
