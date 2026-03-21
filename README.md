<h1 align="center">YantrikClaw</h1>

<p align="center">
  <strong>Personal AI assistant with cognitive memory, tier-aware tools, and 20+ messaging channels.</strong>
</p>

<p align="center">
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache%202.0-blue.svg" alt="License: MIT OR Apache-2.0" /></a>
  <a href="https://github.com/yantrikos/yantrikclaw"><img src="https://img.shields.io/github/stars/yantrikos/yantrikclaw?style=flat" alt="Stars" /></a>
</p>

<p align="center">
  <em>Fork of <a href="https://github.com/zeroclaw-labs/zeroclaw">ZeroClaw</a> with the <a href="https://github.com/yantrikos">Yantrik</a> cognitive stack.</em>
</p>

---

## What is YantrikClaw?

YantrikClaw takes [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) — a fast, Rust-based personal AI assistant with 20+ messaging channels — and adds:

- **Cognitive Memory (YantrikDB)** — persistent memory that survives restarts, tracks entities, relationships, beliefs, and patterns
- **Tier-Aware Tool Selection** — adapts tool presentation based on model size (0.5B → 100B+). Small models get MCQ selection with embedding-ranked candidates; large models get full tool sets
- **Companion Integration** — companion process lifecycle management, proactive urge delivery, personality evolution
- **SearXNG Search** — self-hosted metasearch (Google, Bing, DuckDuckGo, Wikipedia, GitHub, StackOverflow) with no API keys needed

### Why this fork?

No other ZeroClaw fork has cognitive memory, model-tier-adaptive tool selection, or works well with sub-1B parameter models on edge devices.

| Capability | ZeroClaw | YantrikClaw |
|---|---|---|
| Channels | 20+ | 20+ (inherited) |
| Tools | 57 | 57 + tier-aware selection |
| Memory | SQLite sessions | YantrikDB (cognitive graph) |
| Small model support | Basic | MCQ tool selection, embedding ranking, budget caps |
| Proactive messages | No | Urge pipeline via companion |
| Web search | DuckDuckGo (API key) | SearXNG (self-hosted, no keys) |

## Install

### One-liner (Linux/macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/yantrikos/yantrikclaw/main/install.sh | sh
```

### Docker (with SearXNG included)

```bash
git clone https://github.com/yantrikos/yantrikclaw.git
cd yantrikclaw
cp docker-compose.yml docker-compose.override.yml  # edit your API key
docker compose up -d
```

This starts both YantrikClaw and a SearXNG instance for web search — no API keys needed.

### From source

```bash
git clone https://github.com/yantrikos/yantrikclaw.git
cd yantrikclaw
cargo build --release
./target/release/zeroclaw onboard
```

## Quick start

```bash
# Interactive setup (provider, channels, workspace)
zeroclaw onboard

# Start the gateway
zeroclaw start

# Talk to the assistant
zeroclaw agent -m "Hello!"

# Check status
zeroclaw status
```

## Model Tier System

YantrikClaw detects your model's capability tier and adapts accordingly:

| Tier | Models | Max Tools | Selection Mode |
|---|---|---|---|
| **Tiny** (0.5–1.5B) | qwen2.5:0.5b, phi-3-mini | 10 | MCQ (A/B/C/D/E) |
| **Small** (1.5–7B) | llama3.2:3b, gemma3:4b | 20 | MCQ + embedding ranking |
| **Medium** (7–14B) | yantrik-9b, qwen2.5:14b | 25 | Structured JSON |
| **Large** (14B+) | qwen3.5:27b, gpt-4o, claude | 30 | Native function calling |

Small models don't see all 57+ tools — they get the top candidates ranked by embedding similarity to the user's query, presented as a simple multiple-choice question.

## Architecture

```
User (Telegram / Discord / Slack / WhatsApp / 20+ channels)
  │
  ▼
YantrikClaw (single Rust binary)
  ├── Channels (ZeroClaw: 20+)
  ├── LLM Providers (Ollama, OpenAI, Anthropic, OpenRouter, etc.)
  ├── Tools (57 with tier-aware selection)
  │   ├── ToolFamily routing (Communicate, Browse, Files, System, etc.)
  │   ├── Embedding similarity ranking
  │   └── MCQ batched selection for small models
  ├── YantrikDB (cognitive memory — entities, relations, beliefs, patterns)
  ├── Companion (optional)
  │   ├── Process lifecycle management
  │   ├── Urge pipeline (proactive messages)
  │   └── Personality evolution
  └── SearXNG (self-hosted web search)
```

## Configuration

YantrikClaw uses the same config format as ZeroClaw (`~/.zeroclaw/config.toml`) with additional sections:

```toml
# Companion (optional — for proactive messaging)
[companion]
enabled = false
url = "http://127.0.0.1:8080"
manage_process = false
proactive_enabled = false
proactive_channel = "telegram"

# Web search — SearXNG is the default (no API keys needed)
[tools.web_search]
provider = "searxng"
searxng_url = "http://localhost:8888"
```

## Channels

All ZeroClaw channels are supported:

WhatsApp, Telegram, Slack, Discord, Signal, iMessage, Matrix, IRC, Email, Bluesky, Nostr, Mattermost, Nextcloud Talk, DingTalk, Lark, QQ, Reddit, LinkedIn, Twitter, MQTT, WeChat Work, WebSocket, and more.

## Attribution

YantrikClaw is a fork of [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw), which is itself a Rust rewrite of [OpenClaw](https://github.com/openclaw-org/openclaw). ZeroClaw was built by students and members of the Harvard, MIT, and Sundai.Club communities.

This fork adds the [Yantrik](https://github.com/yantrikos) cognitive stack: YantrikDB memory, tier-aware tool selection, companion integration, and SearXNG search.

Licensed under MIT OR Apache-2.0, same as the upstream project.

## License

Dual-licensed under [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE), same as ZeroClaw.
