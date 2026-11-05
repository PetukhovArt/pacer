# syntax=docker/dockerfile:1.7
ARG NODE_VERSION=22
ARG RUST_VERSION=1.98.1
ARG DEBIAN=bookworm

FROM node:${NODE_VERSION}-${DEBIAN}-slim AS web
ENV HUSKY=0 CI=1
RUN npm install -g pnpm@10.33.0
WORKDIR /src
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml tsconfig.base.json tsconfig.json ./
COPY packages packages
COPY apps/desktop/package.json apps/desktop/
COPY apps/mcp/package.json apps/mcp/
COPY apps/model-router/package.json apps/model-router/
COPY apps/runner/package.json apps/runner/
COPY apps/server/package.json apps/server/
COPY apps/web/package.json apps/web/
RUN --mount=type=cache,target=/root/.local/share/pnpm/store pnpm install --frozen-lockfile
COPY apps/web apps/web
RUN pnpm build:tokens && pnpm --filter @veyra/web build

FROM rust:${RUST_VERSION}-${DEBIAN} AS core
WORKDIR /src/apps/desktop/src-tauri
COPY apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/Cargo.lock apps/desktop/src-tauri/build.rs ./
COPY apps/desktop/src-tauri/src src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/apps/desktop/src-tauri/target \
    cargo build --release --locked --no-default-features --bin veyra-server \
    && install -m 0755 target/release/veyra-server /usr/local/bin/veyra-server

FROM rust:${RUST_VERSION}-${DEBIAN} AS pacer
ARG PACER_REPO=https://github.com/PetukhovArt/pacer
ARG PACER_REF=v0.19.0
RUN git clone --depth 1 --branch "${PACER_REF}" "${PACER_REPO}" /pacer
WORKDIR /pacer
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/pacer/target \
    cargo build --release --locked -p pacer \
    && install -m 0755 target/release/pacer /usr/local/bin/pacer

FROM node:${NODE_VERSION}-${DEBIAN}-slim AS runtime
ARG CLAUDE_CODE_VERSION=latest
ARG GLAB_VERSION=
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl git gnupg openssh-client procps tini less \
    && install -d -m 0755 /etc/apt/keyrings \
    && curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg -o /etc/apt/keyrings/githubcli.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli.gpg] https://cli.github.com/packages stable main" > /etc/apt/sources.list.d/github-cli.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends gh \
    && if [ -n "${GLAB_VERSION}" ]; then \
         curl -fsSL -o /tmp/glab.deb "https://gitlab.com/gitlab-org/cli/-/releases/v${GLAB_VERSION}/downloads/glab_${GLAB_VERSION}_linux_$(dpkg --print-architecture).deb" \
         && apt-get install -y /tmp/glab.deb && rm /tmp/glab.deb; \
       fi \
    && rm -rf /var/lib/apt/lists/*
RUN npm install -g "@anthropic-ai/claude-code@${CLAUDE_CODE_VERSION}" && npm cache clean --force
COPY --from=core /usr/local/bin/veyra-server /usr/local/bin/veyra-server
COPY --from=pacer /usr/local/bin/pacer /usr/local/bin/pacer
COPY --from=web /src/apps/web/dist /opt/veyra/web
COPY docker/entrypoint.sh /usr/local/bin/veyra-entrypoint
RUN chmod 0755 /usr/local/bin/veyra-entrypoint && mkdir -p /workspace
ENV VEYRA_BIND=0.0.0.0 \
    VEYRA_PORT=4477 \
    VEYRA_HOME=/root/.veyra \
    VEYRA_WEB_DIR=/opt/veyra/web \
    VEYRA_SECRETS=file \
    PACER_RUNTIME_DIR=/run/pacer \
    CLAUDE_CONFIG_DIR=/root/.claude \
    GIT_CONFIG_GLOBAL=/root/.config/git/config
WORKDIR /workspace
EXPOSE 4477
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -fsS "http://127.0.0.1:${VEYRA_PORT}/__veyra/health" || exit 1
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/bin/tini", "-g", "--", "/usr/local/bin/veyra-entrypoint"]
