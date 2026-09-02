#!/bin/sh
# pacer installer — installs or updates the `pacer` binary.
#
#   curl -fsSL https://raw.githubusercontent.com/PetukhovArt/pacer/main/install.sh | sh
#
# Grabs the prebuilt binary for this platform from the latest GitHub release,
# falling back to `cargo install --git` when no release (or no matching asset)
# exists. Running it again updates in place.
#
# Environment overrides:
#   NEBULA_INSTALL_DIR   install destination (default: ~/.local/bin)
set -eu

REPO="PetukhovArt/pacer"
INSTALL_DIR="${NEBULA_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*"; }
err() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

detect_target() {
    case "$(uname -s)" in
    Darwin)
        case "$(uname -m)" in
        arm64) echo aarch64-apple-darwin ;;
        x86_64) echo x86_64-apple-darwin ;;
        *) return 1 ;;
        esac
        ;;
    Linux)
        case "$(uname -m)" in
        x86_64) echo x86_64-unknown-linux-musl ;;
        aarch64 | arm64) echo aarch64-unknown-linux-musl ;;
        *) return 1 ;;
        esac
        ;;
    *) err "pacer runs on macOS and Linux only (the daemon needs unix sockets)" ;;
    esac
}

install_from_release() {
    url="https://github.com/$REPO/releases/latest/download/pacer-$1.tar.gz"
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    say "downloading $url"
    curl -fsSL "$url" -o "$tmp/pacer.tar.gz" || return 1
    tar -xzf "$tmp/pacer.tar.gz" -C "$tmp"
    mkdir -p "$INSTALL_DIR"
    install -m 755 "$tmp/pacer" "$INSTALL_DIR/pacer"
}

install_from_source() {
    command -v cargo >/dev/null 2>&1 ||
        err "no prebuilt binary available and cargo is not installed — get Rust from https://rustup.rs and re-run"
    say "building from source (this takes a few minutes)…"
    cargo install --git "https://github.com/$REPO" pacer --locked --force
}

main() {
    command -v curl >/dev/null 2>&1 || err "curl is required"

    installed=""
    if target=$(detect_target); then
        if install_from_release "$target"; then
            installed="$INSTALL_DIR/pacer"
        else
            reason="couldn't download pacer-$target from the latest release"
        fi
    else
        reason="no prebuilt binary for $(uname -s) $(uname -m)"
    fi

    if [ -z "$installed" ]; then
        say "$reason — building from source instead"
        install_from_source
        installed="$HOME/.cargo/bin/pacer"
    fi

    version=$("$installed" --version 2>/dev/null || echo pacer)
    say "installed $version → $installed"

    bin_dir=$(dirname "$installed")
    case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) say "note: $bin_dir is not on your PATH — add: export PATH=\"$bin_dir:\$PATH\"" ;;
    esac

    # Replacing the file doesn't touch an already-running daemon: sessions keep
    # running on the old binary until it's restarted, and `pacer kill` is the
    # user's call because it stops every session. `pacer upgrade` handles this
    # itself (shutting down an idle daemon) and suppresses the note via
    # NEBULA_UPGRADE_HANDOFF.
    if [ -z "${NEBULA_UPGRADE_HANDOFF:-}" ] && pgrep -f 'pacer daemon' >/dev/null 2>&1; then
        say "note: a pacer daemon from the previous version is still running."
        say "      run 'pacer kill' to restart onto the new one (stops all sessions)."
    fi
}

main
