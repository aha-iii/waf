# Nerve WAF — developer & release tasks (GNU Make)
# Run from product root: Nerve-waf/Nerve-waf

VERSION := $(shell cat VERSION 2>/dev/null || echo 0.0.0)
PRODUCT_ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
DEPLOY := $(PRODUCT_ROOT)/deploy
HELM_CHART := $(DEPLOY)/helm/nerve-waf
IMAGE ?= ghcr.io/$(shell echo $(GITHUB_REPOSITORY) | tr '[:upper:]' '[:lower:]')/nerve-waf:$(VERSION)

.PHONY: help version install build test fmt clippy helm-lint helm-template docker-build smoke \
        migrate install-tools

help:
	@echo "Nerve WAF $(VERSION)"
	@echo ""
	@echo "  make install        One-shot Docker install (deploy/install.sh)"
	@echo "  make build          Release-build controller + agent + console"
	@echo "  make test           cargo test (controller store)"
	@echo "  make fmt            cargo fmt --all"
	@echo "  make clippy         clippy on controller + agent crates"
	@echo "  make helm-lint      helm lint chart"
	@echo "  make helm-template  render chart to /tmp/nerve-waf.yaml"
	@echo "  make docker-build   standalone image (no Nerve-ui)"
	@echo "  make smoke          scripts/smoke-test.sh (stack must be running)"
	@echo "  make migrate        apply-migrations.sh (needs DATABASE_URL)"

version:
	@echo $(VERSION)

install:
	bash $(DEPLOY)/install.sh

build:
	cd $(PRODUCT_ROOT) && cargo build --release \
		-p nerve-waf-controller -p nerve-waf-agent -p nerve-waf-console \
		--bin nerve-waf-controller --bin nerve-waf-agent --bin nerve-waf-console

test:
	cd $(PRODUCT_ROOT) && cargo test -p nerve-waf-store --lib
	cd $(PRODUCT_ROOT) && cargo test -p nerve-waf-controller -- policy_sim incident baseline

fmt:
	cd $(PRODUCT_ROOT) && cargo fmt --all

clippy:
	cd $(PRODUCT_ROOT) && cargo clippy -p nerve-waf-controller -p nerve-waf-agent -- -D warnings

helm-lint:
	helm lint $(HELM_CHART)

helm-template:
	helm template nerve-waf $(HELM_CHART) --namespace nerve-waf > /tmp/nerve-waf.yaml
	@echo "written /tmp/nerve-waf.yaml"

docker-build:
	cd $(PRODUCT_ROOT) && docker build \
		-f deploy/docker/Dockerfile.standalone \
		-t nerve/nerve-waf-controller:$(VERSION) \
		-t nerve/nerve-waf:$(VERSION) \
		.

smoke:
	bash $(PRODUCT_ROOT)/scripts/smoke-test.sh

migrate:
	bash $(PRODUCT_ROOT)/scripts/apply-migrations.sh
