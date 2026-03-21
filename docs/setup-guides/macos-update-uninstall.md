# macOS Update and Uninstall Guide

This page documents supported update and uninstall procedures for YantrikClaw on macOS (OS X).

Last verified: **February 22, 2026**.

## 1) Check current install method

```bash
which yantrikclaw
yantrikclaw --version
```

Typical locations:

- Homebrew: `/opt/homebrew/bin/yantrikclaw` (Apple Silicon) or `/usr/local/bin/yantrikclaw` (Intel)
- Cargo/bootstrap/manual: `~/.cargo/bin/yantrikclaw`

If both exist, your shell `PATH` order decides which one runs.

## 2) Update on macOS

### A) Homebrew install

```bash
brew update
brew upgrade yantrikclaw
yantrikclaw --version
```

### B) Clone + bootstrap install

From your local repository checkout:

```bash
git pull --ff-only
./install.sh --prefer-prebuilt
yantrikclaw --version
```

If you want source-only update:

```bash
git pull --ff-only
cargo install --path . --force --locked
yantrikclaw --version
```

### C) Manual prebuilt binary install

Re-run your download/install flow with the latest release asset, then verify:

```bash
yantrikclaw --version
```

## 3) Uninstall on macOS

### A) Stop and remove background service first

This prevents the daemon from continuing to run after binary removal.

```bash
yantrikclaw service stop || true
yantrikclaw service uninstall || true
```

Service artifacts removed by `service uninstall`:

- `~/Library/LaunchAgents/com.yantrikclaw.daemon.plist`

### B) Remove the binary by install method

Homebrew:

```bash
brew uninstall yantrikclaw
```

Cargo/bootstrap/manual (`~/.cargo/bin/yantrikclaw`):

```bash
cargo uninstall yantrikclaw || true
rm -f ~/.cargo/bin/yantrikclaw
```

### C) Optional: remove local runtime data

Only run this if you want a full cleanup of config, auth profiles, logs, and workspace state.

```bash
rm -rf ~/.yantrikclaw
```

## 4) Verify uninstall completed

```bash
command -v yantrikclaw || echo "yantrikclaw binary not found"
pgrep -fl yantrikclaw || echo "No running yantrikclaw process"
```

If `pgrep` still finds a process, stop it manually and re-check:

```bash
pkill -f yantrikclaw
```

## Related docs

- [One-Click Bootstrap](one-click-bootstrap.md)
- [Commands Reference](../reference/cli/commands-reference.md)
- [Troubleshooting](../ops/troubleshooting.md)
