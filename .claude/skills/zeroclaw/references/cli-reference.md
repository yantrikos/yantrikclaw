# YantrikClaw CLI Reference

Complete command reference for the `yantrikclaw` binary.

## Table of Contents

1. [Agent](#agent)
2. [Onboarding](#onboarding)
3. [Status & Diagnostics](#status--diagnostics)
4. [Memory](#memory)
5. [Cron](#cron)
6. [Providers & Models](#providers--models)
7. [Gateway & Daemon](#gateway--daemon)
8. [Service Management](#service-management)
9. [Channels](#channels)
10. [Security & Emergency Stop](#security--emergency-stop)
11. [Hardware Peripherals](#hardware-peripherals)
12. [Skills](#skills)
13. [Shell Completions](#shell-completions)

---

## Agent

Interactive chat or single-message mode.

```bash
yantrikclaw agent                                          # Interactive REPL
yantrikclaw agent -m "Summarize today's logs"              # Single message
yantrikclaw agent -p anthropic --model claude-sonnet-4-6   # Override provider/model
yantrikclaw agent -t 0.3                                   # Set temperature
yantrikclaw agent --peripheral nucleo-f401re:/dev/ttyACM0  # Attach hardware
```

**Key flags:**
- `-m <message>` — single message mode (no REPL)
- `-p <provider>` — override provider (openrouter, anthropic, openai, ollama)
- `--model <model>` — override model
- `-t <float>` — temperature (0.0–2.0)
- `--peripheral <name>:<port>` — attach hardware peripheral

The agent has access to 30+ tools gated by security policy: shell, file_read, file_write, file_edit, glob_search, content_search, memory_store, memory_recall, memory_forget, browser, http_request, web_fetch, web_search, cron, delegate, git, and more. Max tool iterations defaults to 10.

---

## Onboarding

First-time setup or reconfiguration.

```bash
yantrikclaw onboard                                 # Quick mode (default: openrouter)
yantrikclaw onboard --provider anthropic            # Quick mode with specific provider
yantrikclaw onboard                                 # Guided wizard (default)
yantrikclaw onboard --memory sqlite                 # Set memory backend
yantrikclaw onboard --force                         # Overwrite existing config
yantrikclaw onboard --channels-only                 # Repair channels only
```

**Key flags:**
- `--provider <name>` — openrouter (default), anthropic, openai, ollama
- `--model <model>` — default model
- `--memory <backend>` — sqlite, markdown, lucid, none
- `--force` — overwrite existing config.toml
- `--channels-only` — only repair channel configuration
- `--reinit` — start fresh (backs up existing config)

Creates `~/.yantrikclaw/config.toml` with `0600` permissions.

---

## Status & Diagnostics

```bash
yantrikclaw status                    # System overview
yantrikclaw doctor                    # Run all diagnostic checks
yantrikclaw doctor models             # Probe model connectivity
yantrikclaw doctor traces             # Query execution traces
```

---

## Memory

```bash
yantrikclaw memory list                              # List all entries
yantrikclaw memory list --category core --limit 10   # Filtered list
yantrikclaw memory get "some-key"                    # Get specific entry
yantrikclaw memory stats                             # Usage statistics
yantrikclaw memory clear --key "prefix" --yes        # Delete entries (requires --yes)
```

**Key flags:**
- `--category <name>` — filter by category (core, daily, conversation, custom)
- `--limit <n>` — limit results
- `--key <prefix>` — key prefix for clear operations
- `--yes` — skip confirmation (required for clear)

---

## Cron

```bash
yantrikclaw cron list                                                      # List all jobs
yantrikclaw cron add '0 9 * * 1-5' 'Good morning' --tz America/New_York   # Recurring (cron expr)
yantrikclaw cron add-at '2026-03-11T10:00:00Z' 'Remind me about meeting'  # One-time at specific time
yantrikclaw cron add-every 3600000 'Check server health'                   # Interval in milliseconds
yantrikclaw cron once 30m 'Follow up on that task'                         # Delay from now
yantrikclaw cron pause <id>                                                # Pause job
yantrikclaw cron resume <id>                                               # Resume job
yantrikclaw cron remove <id>                                               # Delete job
```

**Subcommands:**
- `add <cron-expr> <command>` — standard cron expression (5-field)
- `add-at <iso-datetime> <command>` — fire once at exact time
- `add-every <ms> <command>` — repeating interval
- `once <duration> <command>` — delay from now (e.g., `30m`, `2h`, `1d`)

---

## Providers & Models

```bash
yantrikclaw providers                                # List all 40+ supported providers
yantrikclaw models list                              # Show cached model catalog
yantrikclaw models refresh --all                     # Refresh catalogs from all providers
yantrikclaw models set anthropic/claude-sonnet-4-6   # Set default model
yantrikclaw models status                            # Current model info
```

Model routing in config.toml:
```toml
[[model_routes]]
hint = "reasoning"
provider = "openrouter"
model = "anthropic/claude-sonnet-4-6"
```

---

## Gateway & Daemon

```bash
yantrikclaw gateway                                 # Start HTTP gateway (foreground)
yantrikclaw gateway -p 8080 --host 127.0.0.1        # Custom port/host

yantrikclaw daemon                                  # Gateway + channels + scheduler + heartbeat
yantrikclaw daemon -p 8080 --host 0.0.0.0           # Custom bind
```

**Gateway defaults:**
- Port: 42617
- Host: 127.0.0.1
- Pairing required: true
- Public bind allowed: false

---

## Service Management

OS service lifecycle (systemd on Linux, launchd on macOS).

```bash
yantrikclaw service install     # Install as system service
yantrikclaw service start       # Start the service
yantrikclaw service status      # Check service status
yantrikclaw service stop        # Stop the service
yantrikclaw service restart     # Restart the service
yantrikclaw service uninstall   # Remove the service
```

**Logs:**
- macOS: `~/.yantrikclaw/logs/daemon.stdout.log`
- Linux: `journalctl -u yantrikclaw`

---

## Channels

Channels are configured in `config.toml` under `[channels]` and `[channels_config.*]`.

```bash
yantrikclaw channels list       # List configured channels
yantrikclaw channels doctor     # Check channel health
```

Supported channels (21 total): Telegram, Discord, Slack, WhatsApp (Meta), WATI, Linq (iMessage/RCS/SMS), Email (IMAP/SMTP), IRC, Matrix, Nostr, Signal, Nextcloud Talk, and more.

Channel config example (Telegram):
```toml
[channels]
telegram = true

[channels_config.telegram]
bot_token = "..."
allowed_users = [123456789]
```

---

## Security & Emergency Stop

```bash
yantrikclaw estop --level kill-all                              # Stop everything
yantrikclaw estop --level network-kill                          # Block all network access
yantrikclaw estop --level domain-block --domain "*.example.com" # Block specific domains
yantrikclaw estop --level tool-freeze --tool shell              # Freeze specific tool
yantrikclaw estop status                                        # Check estop state
yantrikclaw estop resume --network                              # Resume (may require OTP)
```

**Estop levels:**
- `kill-all` — nuclear option, stops all agent activity
- `network-kill` — blocks all outbound network
- `domain-block` — blocks specific domain patterns
- `tool-freeze` — freezes individual tools

Autonomy config in config.toml:
```toml
[autonomy]
level = "supervised"                           # read_only | supervised | full
workspace_only = true
allowed_commands = ["git", "cargo", "python"]
forbidden_paths = ["/etc", "/root", "~/.ssh"]
max_actions_per_hour = 20
max_cost_per_day_cents = 500
```

---

## Hardware Peripherals

```bash
yantrikclaw hardware discover                              # Find USB devices
yantrikclaw hardware introspect /dev/ttyACM0               # Probe device capabilities
yantrikclaw peripheral list                                # List configured peripherals
yantrikclaw peripheral add nucleo-f401re /dev/ttyACM0      # Add peripheral
yantrikclaw peripheral flash-nucleo                        # Flash STM32 firmware
yantrikclaw peripheral flash --port /dev/cu.usbmodem101    # Flash Arduino firmware
```

**Supported boards:** STM32 Nucleo-F401RE, Arduino Uno R4, Raspberry Pi GPIO, ESP32.

Attach to agent session: `yantrikclaw agent --peripheral nucleo-f401re:/dev/ttyACM0`

---

## Skills

```bash
yantrikclaw skills list         # List installed skills
yantrikclaw skills install <path-or-url>  # Install a skill
yantrikclaw skills audit        # Audit installed skills
yantrikclaw skills remove <name>  # Remove a skill
```

---

## Shell Completions

```bash
yantrikclaw completions zsh     # Generate Zsh completions
yantrikclaw completions bash    # Generate Bash completions
yantrikclaw completions fish    # Generate Fish completions
```

---

## Config File

Default location: `~/.yantrikclaw/config.toml`

Config resolution order (first match wins):
1. `YANTRIKCLAW_CONFIG_DIR` environment variable
2. `YANTRIKCLAW_WORKSPACE` environment variable
3. `~/.yantrikclaw/active_workspace.toml` marker file
4. `~/.yantrikclaw/config.toml` (default)
