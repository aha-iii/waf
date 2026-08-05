# 构建上下文：仓库根目录 waf/（含 Nerve-alert、Nerve-ui、Nerve-waf-native）
#
#   docker build -f Nerve-waf/deploy/docker/Dockerfile -t nerve/nerve-waf-api:1.0.0 .

FROM --platform=$BUILDPLATFORM node:20-bookworm AS console-build

ARG DEBIAN_MIRROR=mirrors.aliyun.com
ARG NPM_REGISTRY=https://registry.npmmirror.com
ARG VITE_CONSOLE_TIME_ZONE=Asia/Shanghai

RUN sed -i "s|deb.debian.org|${DEBIAN_MIRROR}|g; s|security.debian.org|${DEBIAN_MIRROR}|g" \
      /etc/apt/sources.list.d/debian.sources 2>/dev/null || \
    sed -i "s|deb.debian.org|${DEBIAN_MIRROR}|g; s|security.debian.org|${DEBIAN_MIRROR}|g" \
      /etc/apt/sources.list

WORKDIR /src
COPY Nerve-ui /src/Nerve-ui
COPY Nerve-waf/apps/console-react/package.json Nerve-waf/apps/console-react/package-lock.json /src/Nerve-waf/apps/console-react/

WORKDIR /src/Nerve-waf/apps/console-react
RUN npm config set registry "${NPM_REGISTRY}" && npm ci

COPY Nerve-waf/apps/console-react /src/Nerve-waf/apps/console-react
ENV VITE_CONSOLE_TIME_ZONE=${VITE_CONSOLE_TIME_ZONE}
RUN npx vite build

FROM --platform=$BUILDPLATFORM rust:bookworm AS build

ARG DEBIAN_MIRROR=mirrors.aliyun.com

RUN sed -i "s|deb.debian.org|${DEBIAN_MIRROR}|g; s|security.debian.org|${DEBIAN_MIRROR}|g" \
      /etc/apt/sources.list.d/debian.sources 2>/dev/null || \
    sed -i "s|deb.debian.org|${DEBIAN_MIRROR}|g; s|security.debian.org|${DEBIAN_MIRROR}|g" \
      /etc/apt/sources.list

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY Nerve-alert /src/Nerve-alert
COPY Nerve-waf-native /src/Nerve-waf-native
COPY Nerve-waf /src/Nerve-waf

WORKDIR /src/Nerve-waf
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release -p nerve-waf-api --bin nerve-waf-api --bin nerve-waf-console -p nerve-waf-edge

RUN mkdir -p /out && cp /src/Nerve-waf/target/release/nerve-waf-api /out/nerve-waf-api && \
    cp /src/Nerve-waf/target/release/nerve-waf-edge /out/nerve-waf-edge && \
    cp /src/Nerve-waf/target/release/nerve-waf-console /out/nerve-waf-console

FROM debian:bookworm-slim

ARG DEBIAN_MIRROR=mirrors.aliyun.com

RUN sed -i "s|deb.debian.org|${DEBIAN_MIRROR}|g; s|security.debian.org|${DEBIAN_MIRROR}|g" \
      /etc/apt/sources.list.d/debian.sources 2>/dev/null || \
    sed -i "s|deb.debian.org|${DEBIAN_MIRROR}|g; s|security.debian.org|${DEBIAN_MIRROR}|g" \
      /etc/apt/sources.list

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl libssl3 tzdata \
    && ln -sf /usr/share/zoneinfo/Asia/Shanghai /etc/localtime \
    && echo "Asia/Shanghai" > /etc/timezone \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /out/nerve-waf-api /usr/local/bin/nerve-waf-api
COPY --from=build /out/nerve-waf-edge /usr/local/bin/nerve-waf-edge
COPY --from=build /out/nerve-waf-console /usr/local/bin/nerve-waf-console
COPY --from=console-build /src/Nerve-waf/apps/console-react/dist /opt/nerve-waf/console
COPY Nerve-waf/deploy/docker/config.docker.yml /etc/nerve-waf/config.yml
COPY Nerve-waf/plugins /opt/nerve-waf/plugins
COPY Nerve-waf/data/rules /opt/nerve-waf/data/rules
COPY Nerve-waf/Nerve-defender/vendor/coreruleset /opt/nerve-waf/vendor/coreruleset
COPY Nerve-waf-native/packs/baseline /opt/nerve/native/baseline

RUN mkdir -p /var/lib/nerve-waf/data /var/lib/nerve-waf/logs && \
    chmod 755 /usr/local/bin/nerve-waf-api /usr/local/bin/nerve-waf-edge /usr/local/bin/nerve-waf-console

ENV TZ=Asia/Shanghai \
    PORT=8781 \
    WAF_CONFIG=/etc/nerve-waf/config.yml \
    NERVE_WAF_DATA=/var/lib/nerve-waf/data \
    NERVE_RULES_DIR=/opt/nerve-waf/data/rules/crs \
    WAF_CONSOLE_ATTACH=0 \
    WAF_CONSOLE_DIR=/opt/nerve-waf/console \
    WAF_CONSOLE_PORT=8782 \
    WAF_API_UPSTREAM=http://127.0.0.1:8781 \
    NERVE_AUTH_ENABLED=1 \
    NERVE_EXPERT_ENGINE=on \
    NERVE_EXPERT_PACK=/opt/nerve/native/baseline \
    NERVE_NATIVE_DEV_PACK=1 \
    WAF_ENABLE_LEGACY_TOPOLOGY=0 \
    WAF_ENABLE_LEGACY_NP=0

EXPOSE 8781 8782 8080

HEALTHCHECK --interval=15s --timeout=5s --start-period=20s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8781/api/health || exit 1

VOLUME ["/var/lib/nerve-waf/data", "/var/lib/nerve-waf/logs"]

ENTRYPOINT ["/usr/local/bin/nerve-waf-api"]
CMD ["--config", "/etc/nerve-waf/config.yml"]
