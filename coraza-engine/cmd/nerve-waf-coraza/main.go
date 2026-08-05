// nerve-waf-coraza — 进程内嵌 Coraza + OWASP CRS v4，对外 HTTP inspect。
//
// Coraza 官方为 Go；Rust WAF/Gateway 通过本进程对齐 ModSecurity/CRS 全集语义。
package main

import (
	"encoding/json"
	"flag"
	"log"
	"net/http"
	"os"
	"strings"
	"time"

	"github.com/nerve/nerve-waf/coraza-engine/internal/engine"
)

func main() {
	addr := flag.String("listen", envOr("NERVE_CORAZA_LISTEN", "127.0.0.1:8791"), "HTTP listen address")
	flag.Parse()

	eng, err := engine.New()
	if err != nil {
		log.Fatalf("coraza engine init: %v", err)
	}
	log.Printf("nerve-waf-coraza: Coraza+CRS embedded, listen http://%s", *addr)

	mux := http.NewServeMux()
	mux.HandleFunc("/healthz", func(w http.ResponseWriter, _ *http.Request) {
		writeJSON(w, http.StatusOK, map[string]any{"ok": true, "engine": "coraza+crs"})
	})
	mux.HandleFunc("/api/inspect", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "POST required", http.StatusMethodNotAllowed)
			return
		}
		var req engine.InspectRequest
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		// headers may arrive as raw JSON object or string from Gateway
		if req.Headers == nil {
			req.Headers = map[string]string{}
		}
		resp := eng.Inspect(req)
		writeJSON(w, http.StatusOK, resp)
	})

	srv := &http.Server{
		Addr:              *addr,
		Handler:           withLog(mux),
		ReadHeaderTimeout: 5 * time.Second,
	}
	if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
		log.Fatal(err)
	}
}

func withLog(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()
		next.ServeHTTP(w, r)
		if r.URL.Path != "/healthz" {
			log.Printf("%s %s %s", r.Method, r.URL.Path, time.Since(start))
		}
	})
}

func writeJSON(w http.ResponseWriter, code int, v any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(code)
	_ = json.NewEncoder(w).Encode(v)
}

func envOr(k, def string) string {
	if v := strings.TrimSpace(os.Getenv(k)); v != "" {
		return v
	}
	return def
}
