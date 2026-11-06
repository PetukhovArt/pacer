# syntax=docker/dockerfile:1.7
ARG NODE_VERSION=22
ARG RUST_VERSION=1.99.1
ARG DEBIAN=bookworm

FROM node:${NODE_VERSION}-${DEBIAN}-slim AS web
ENV HUSKY=0 CI=1
RUN npm install -g pnpm@10.33.0
WORKDIR /src
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml tsconfig.base.json tsconfig.json ./
COPY packages packages
COPY apps/desktop/package.json apps/desktop/
COPY apps/web/package.json apps/web/
RUN --mount=type=cache,target=/root/.local/share/pnpm/store pnpm install --frozen-lockfile
COPY apps/web apps/web
RUN pnpm build:tokens && pnpm --filter @berth/web build

FROM rust:${RUST_VERSION}-${DEBIAN} AS core
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY vendor vendor
COPY apps/desktop/src-tauri apps/desktop/src-tauri
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p berth \
    && cargo build --release --locked -p berth-desktop --no-default-features --bin berth-server \
    && install -m 0755 target/release/berth target/release/berth-server /usr/local/bin/

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
COPY --from=core /usr/local/bin/berth /usr/local/bin/berth
COPY --from=core /usr/local/bin/berth-server /usr/local/bin/berth-server
COPY --from=web /src/apps/web/dist /opt/berth/web
COPY docker/entrypoint.sh /usr/local/bin/berth-entrypoint
RUN chmod 0755 /usr/local/bin/berth-entrypoint && mkdir -p /workspace
ENV BERTH_BIND=0.0.0.0 \
    BERTH_PORT=4477 \
    BERTH_HOME=/root/.berth \
    BERTH_WEB_DIR=/opt/berth/web \
    BERTH_SECRETS=file \
    BERTH_RUNTIME_DIR=/run/berth \
    CLAUDE_CONFIG_DIR=/root/.claude \
    GIT_CONFIG_GLOBAL=/root/.config/git/config
WORKDIR /workspace
EXPOSE 4477
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -fsS "http://127.0.0.1:${BERTH_PORT}/__berth/health" || exit 1
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/bin/tini", "-g", "--", "/usr/local/bin/berth-entrypoint"]
