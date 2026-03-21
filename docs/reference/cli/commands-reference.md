# YantrikClaw Commands Reference

This reference is derived from the current CLI surface (`yantrikclaw --help`).

Last verified: **February 21, 2026**.

## Top-Level Commands

| Command | Purpose |
|---|---|
| `onboard` | Initialize workspace/config quickly or interactively |
| `agent` | Run interactive chat or single-message mode |
| `gateway` | Start webhook and WhatsApp HTTP gateway |
| `daemon` | Start supervised runtime (gateway + channels + optional heartbeat/scheduler) |
| `service` | Manage user-level OS service lifecycle |
| `doctor` | Run diagnostics and freshness checks |
| `status` | Print current configuration and system summary |
| `estop` | Engage/resume emergency stop levels and inspect estop state |
| `cron` | Manage scheduled tasks |
| `models` | Refresh provider model catalogs |
| `providers` | List provider IDs, aliases, and active provider |
| `channel` | Manage channels and channel health checks |
| `integrations` | Inspect integration details |
| `skills` | List/install/remove skills |
| `migrate` | Import from external runtimes (currently OpenClaw) |
| `config` | Export machine-readable config schema |
| `completions` | Generate shell completion scripts to stdout |
| `hardware` | Discover and introspect USB hardware |
| `peripheral` | Configure and flash peripherals |

## Command Groups

### `onboard`

- `yantrikclaw onboard`
- `yantrikclaw onboard --channels-only`
- `yantrikclaw onboard --force`
- `yantrikclaw onboard --reinit`
- `yantrikclaw onboard --api-key <KEY> --provider <ID> --memory <sqlite|lucid|markdown|none>`
- `yantrikclaw onboard --api-key <KEY> --provider <ID> --model <MODEL_ID> --memory <sqlite|lucid|markdown|none>`
- `yantrikclaw onboard --api-key <KEY> --provider <ID> --model <MODEL_ID> --memory <sqlite|lucid|markdown|none> --force`

`onboard` safety behavior:

- If `config.toml` already exists, onboarding offers two modes:
  - Full onboarding (overwrite `config.toml`)
  - Provider-only update (update provider/model/API key while preserving existing channels, tunnel, memory, hooks, and other settings)
- In non-interactive environments, existing `config.toml` causes a safe refusal unless `--force` is passed.
- Use `yantrikclaw onboard --channels-only` when you only need to rotate channel tokens/allowlists.
- Use `yantrikclaw onboard --reinit` to start fresh. This backs up your existing config directory with a timestamp suffix and creates a new configuration from scratch.

### `agent`

- `yantrikclaw agent`
- `yantrikclaw agent -m "Hello"`
- `yantrikclaw agent --provider <ID> --model <MODEL> --temperature <0.0-2.0>`
- `yantrikclaw agent --peripheral <board:path>`

Tip:

- In interactive chat, you can ask for route changes in natural language (for example “conversation uses kimi, coding uses gpt-5.3-codex”); the assistant can persist this via tool `model_routing_config`.

### `gateway` / `daemon`

- `yantrikclaw gateway [--host <HOST>] [--port <PORT>]`
- `yantrikclaw daemon [--host <HOST>] [--port <PORT>]`

### `estop`

- `yantrikclaw estop` (engage `kill-all`)
- `yantrikclaw estop --level network-kill`
- `yantrikclaw estop --level domain-block --domain "*.chase.com" [--domain "*.paypal.com"]`
- `yantrikclaw estop --level tool-freeze --tool shell [--tool browser]`
- `yantrikclaw estop status`
- `yantrikclaw estop resume`
- `yantrikclaw estop resume --network`
- `yantrikclaw estop resume --domain "*.chase.com"`
- `yantrikclaw estop resume --tool shell`
- `yantrikclaw estop resume --otp <123456>`

Notes:

- `estop` commands require `[security.estop].enabled = true`.
- When `[security.estop].require_otp_to_resume = true`, `resume` requires OTP validation.
- OTP prompt appears automatically if `--otp` is omitted.

### `service`

- `yantrikclaw service install`
- `yantrikclaw service start`
- `yantrikclaw service stop`
- `yantrikclaw service restart`
- `yantrikclaw service status`
- `yantrikclaw service uninstall`

### `cron`

- `yantrikclaw cron list`
- `yantrikclaw cron add <expr> [--tz <IANA_TZ>] <command>`
- `yantrikclaw cron add-at <rfc3339_timestamp> <command>`
- `yantrikclaw cron add-every <every_ms> <command>`
- `yantrikclaw cron once <delay> <command>`
- `yantrikclaw cron remove <id>`
- `yantrikclaw cron pause <id>`
- `yantrikclaw cron resume <id>`

Notes:

- Mutating schedule/cron actions require `cron.enabled = true`.
- Shell command payloads for schedule creation (`create` / `add` / `once`) are validated by security command policy before job persistence.

### `models`

- `yantrikclaw models refresh`
- `yantrikclaw models refresh --provider <ID>`
- `yantrikclaw models refresh --force`

`models refresh` currently supports live catalog refresh for provider IDs: `openrouter`, `openai`, `anthropic`, `groq`, `mistral`, `deepseek`, `xai`, `together-ai`, `gemini`, `ollama`, `llamacpp`, `sglang`, `vllm`, `astrai`, `venice`, `fireworks`, `cohere`, `moonshot`, `glm`, `zai`, `qwen`, and `nvidia`.

### `doctor`

- `yantrikclaw doctor`
- `yantrikclaw doctor models [--provider <ID>] [--use-cache]`
- `yantrikclaw doctor traces [--limit <N>] [--event <TYPE>] [--contains <TEXT>]`
- `yantrikclaw doctor traces --id <TRACE_ID>`

`doctor traces` reads runtime tool/model diagnostics from `observability.runtime_trace_path`.

### `channel`

- `yantrikclaw channel list`
- `yantrikclaw channel start`
- `yantrikclaw channel doctor`
- `yantrikclaw channel bind-telegram <IDENTITY>`
- `yantrikclaw channel add <type> <json>`
- `yantrikclaw channel remove <name>`

Runtime in-chat commands (Telegram/Discord while channel server is running):

- `/models`
- `/models <provider>`
- `/model`
- `/model <model-id>`
- `/new`

Channel runtime also watches `config.toml` and hot-applies updates to:
- `default_provider`
- `default_model`
- `default_temperature`
- `api_key` / `api_url` (for the default provider)
- `reliability.*` provider retry settings

`add/remove` currently route you back to managed setup/manual config paths (not full declarative mutators yet).

### `integrations`

- `yantrikclaw integrations info <name>`

### `skills`

- `yantrikclaw skills list`
- `yantrikclaw skills audit <source_or_name>`
- `yantrikclaw skills install <source>`
- `yantrikclaw skills remove <name>`

`<source>` accepts git remotes (`https://...`, `http://...`, `ssh://...`, and `git@host:owner/repo.git`) or a local filesystem path.

`skills install` always runs a built-in static security audit before the skill is accepted. The audit blocks:
- symlinks inside the skill package
- script-like files (`.sh`, `.bash`, `.zsh`, `.ps1`, `.bat`, `.cmd`)
- high-risk command snippets (for example pipe-to-shell payloads)
- markdown links that escape the skill root, point to remote markdown, or target script files

Use `skills audit` to manually validate a candidate skill directory (or an installed skill by name) before sharing it.

Skill manifests (`SKILL.toml`) support `prompts` and `[[tools]]`; both are injected into the agent system prompt at runtime, so the model can follow skill instructions without manually reading skill files.

### `migrate`

- `yantrikclaw migrate openclaw [--source <path>] [--dry-run]`

### `config`

- `yantrikclaw config schema`

`config schema` prints a JSON Schema (draft 2020-12) for the full `config.toml` contract to stdout.

### `completions`

- `yantrikclaw completions bash`
- `yantrikclaw completions fish`
- `yantrikclaw completions zsh`
- `yantrikclaw completions powershell`
- `yantrikclaw completions elvish`

`completions` is stdout-only by design so scripts can be sourced directly without log/warning contamination.

### `hardware`

- `yantrikclaw hardware discover`
- `yantrikclaw hardware introspect <path>`
- `yantrikclaw hardware info [--chip <chip_name>]`

### `peripheral`

- `yantrikclaw peripheral list`
- `yantrikclaw peripheral add <board> <path>`
- `yantrikclaw peripheral flash [--port <serial_port>]`
- `yantrikclaw peripheral setup-uno-q [--host <ip_or_host>]`
- `yantrikclaw peripheral flash-nucleo`

## Validation Tip

To verify docs against your current binary quickly:

```bash
yantrikclaw --help
yantrikclaw <command> --help
```
