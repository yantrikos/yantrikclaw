# One-Click Bootstrap

This page defines the fastest supported path to install and initialize YantrikClaw.

Last verified: **February 20, 2026**.

## Option 0: Homebrew (macOS/Linuxbrew)

```bash
brew install yantrikclaw
```

## Option A (Recommended): Clone + local script

```bash
git clone https://github.com/yantrikclaw-labs/yantrikclaw.git
cd yantrikclaw
./install.sh
```

What it does by default:

1. `cargo build --release --locked`
2. `cargo install --path . --force --locked`

### Resource preflight and pre-built flow

Source builds typically require at least:

- **2 GB RAM + swap**
- **6 GB free disk**

When resources are constrained, bootstrap now attempts a pre-built binary first.

```bash
./install.sh --prefer-prebuilt
```

To require binary-only installation and fail if no compatible release asset exists:

```bash
./install.sh --prebuilt-only
```

To bypass pre-built flow and force source compilation:

```bash
./install.sh --force-source-build
```

## Dual-mode bootstrap

Default behavior is **app-only** (build/install YantrikClaw) and expects existing Rust toolchain.

For fresh machines, enable environment bootstrap explicitly:

```bash
./install.sh --install-system-deps --install-rust
```

Notes:

- `--install-system-deps` installs compiler/build prerequisites (may require `sudo`).
- `--install-rust` installs Rust via `rustup` when missing.
- `--prefer-prebuilt` tries release binary download first, then falls back to source build.
- `--prebuilt-only` disables source fallback.
- `--force-source-build` disables pre-built flow entirely.

## Option B: Remote one-liner

```bash
curl -fsSL https://raw.githubusercontent.com/yantrikclaw-labs/yantrikclaw/master/install.sh | bash
```

For high-security environments, prefer Option A so you can review the script before execution.

If you run Option B outside a repository checkout, the install script automatically clones a temporary workspace, builds, installs, and then cleans it up.

## Optional onboarding modes

### Containerized onboarding (Docker)

```bash
./install.sh --docker
```

This builds a local YantrikClaw image and launches onboarding inside a container while
persisting config/workspace to `./.yantrikclaw-docker`.

Container CLI defaults to `docker`. If Docker CLI is unavailable and `podman` exists,
the installer auto-falls back to `podman`. You can also set `YANTRIKCLAW_CONTAINER_CLI`
explicitly (for example: `YANTRIKCLAW_CONTAINER_CLI=podman ./install.sh --docker`).

For Podman, the installer runs with `--userns keep-id` and `:Z` volume labels so
workspace/config mounts remain writable inside the container.

If you add `--skip-build`, the installer skips local image build. It first tries the local
Docker tag (`YANTRIKCLAW_DOCKER_IMAGE`, default: `yantrikclaw-bootstrap:local`); if missing,
it pulls `ghcr.io/yantrikclaw-labs/yantrikclaw:latest` and tags it locally before running.

### Stopping and restarting a Docker/Podman container

After `./install.sh --docker` finishes, the container exits. Your config and workspace
are persisted in the data directory (default: `./.yantrikclaw-docker`, or `~/.yantrikclaw-docker`
when bootstrapping via `curl | bash`). You can override this path with `YANTRIKCLAW_DOCKER_DATA_DIR`.

**Do not re-run `install.sh`** to restart -- it will rebuild the image and re-run onboarding.
Instead, start a new container from the existing image and mount the persisted data directory.

#### Using the repository docker-compose.yml

The simplest way to run YantrikClaw long-term in Docker/Podman is with the provided
`docker-compose.yml` at the repository root. It uses a named volume (`yantrikclaw-data`)
and sets `restart: unless-stopped` so the container survives reboots.

```bash
# Start (detached)
docker compose up -d

# Stop
docker compose down

# Restart after stopping
docker compose up -d
```

Replace `docker` with `podman` if you use Podman.

#### Manual container run (using install.sh data directory)

If you installed via `./install.sh --docker` and want to reuse the `.yantrikclaw-docker`
data directory without compose:

```bash
# Docker
docker run -d --name yantrikclaw \
  --restart unless-stopped \
  -v "$PWD/.yantrikclaw-docker/.yantrikclaw:/yantrikclaw-data/.yantrikclaw" \
  -v "$PWD/.yantrikclaw-docker/workspace:/yantrikclaw-data/workspace" \
  -e HOME=/yantrikclaw-data \
  -e YANTRIKCLAW_WORKSPACE=/yantrikclaw-data/workspace \
  -p 42617:42617 \
  yantrikclaw-bootstrap:local \
  gateway

# Podman (add --userns keep-id and :Z volume labels)
podman run -d --name yantrikclaw \
  --restart unless-stopped \
  --userns keep-id \
  --user "$(id -u):$(id -g)" \
  -v "$PWD/.yantrikclaw-docker/.yantrikclaw:/yantrikclaw-data/.yantrikclaw:Z" \
  -v "$PWD/.yantrikclaw-docker/workspace:/yantrikclaw-data/workspace:Z" \
  -e HOME=/yantrikclaw-data \
  -e YANTRIKCLAW_WORKSPACE=/yantrikclaw-data/workspace \
  -p 42617:42617 \
  yantrikclaw-bootstrap:local \
  gateway
```

#### Common lifecycle commands

```bash
# Stop the container (preserves data)
docker stop yantrikclaw

# Start a stopped container (config and workspace are intact)
docker start yantrikclaw

# View logs
docker logs -f yantrikclaw

# Remove the container (data in volumes/.yantrikclaw-docker is preserved)
docker rm yantrikclaw

# Check health
docker exec yantrikclaw yantrikclaw status
```

#### Environment variables

When running manually, pass provider configuration as environment variables
or ensure they are already saved in the persisted `config.toml`:

```bash
docker run -d --name yantrikclaw \
  -e API_KEY="sk-..." \
  -e PROVIDER="openrouter" \
  -v "$PWD/.yantrikclaw-docker/.yantrikclaw:/yantrikclaw-data/.yantrikclaw" \
  -v "$PWD/.yantrikclaw-docker/workspace:/yantrikclaw-data/workspace" \
  -p 42617:42617 \
  yantrikclaw-bootstrap:local \
  gateway
```

If you already ran `onboard` during the initial install, your API key and provider are
saved in `.yantrikclaw-docker/.yantrikclaw/config.toml` and do not need to be passed again.

### Quick onboarding (non-interactive)

```bash
./install.sh --api-key "sk-..." --provider openrouter
```

Or with environment variables:

```bash
YANTRIKCLAW_API_KEY="sk-..." YANTRIKCLAW_PROVIDER="openrouter" ./install.sh
```

## Useful flags

- `--install-system-deps`
- `--install-rust`
- `--skip-build` (in `--docker` mode: use local image if present, otherwise pull `ghcr.io/yantrikclaw-labs/yantrikclaw:latest`)
- `--skip-install`
- `--provider <id>`

See all options:

```bash
./install.sh --help
```

## Related docs

- [README.md](../README.md)
- [commands-reference.md](../reference/cli/commands-reference.md)
- [providers-reference.md](../reference/api/providers-reference.md)
- [channels-reference.md](../reference/api/channels-reference.md)
