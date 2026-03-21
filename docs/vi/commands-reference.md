# Tham khảo lệnh YantrikClaw

Dựa trên CLI hiện tại (`yantrikclaw --help`).

Xác minh lần cuối: **2026-02-20**.

## Lệnh cấp cao nhất

| Lệnh | Mục đích |
|---|---|
| `onboard` | Khởi tạo workspace/config nhanh hoặc tương tác |
| `agent` | Chạy chat tương tác hoặc chế độ gửi tin nhắn đơn |
| `gateway` | Khởi động gateway webhook và HTTP WhatsApp |
| `daemon` | Khởi động runtime có giám sát (gateway + channels + heartbeat/scheduler tùy chọn) |
| `service` | Quản lý vòng đời dịch vụ cấp hệ điều hành |
| `doctor` | Chạy chẩn đoán và kiểm tra trạng thái |
| `status` | Hiển thị cấu hình và tóm tắt hệ thống |
| `cron` | Quản lý tác vụ định kỳ |
| `models` | Làm mới danh mục model của provider |
| `providers` | Liệt kê ID provider, bí danh và provider đang dùng |
| `channel` | Quản lý kênh và kiểm tra sức khỏe kênh |
| `integrations` | Kiểm tra chi tiết tích hợp |
| `skills` | Liệt kê/cài đặt/gỡ bỏ skills |
| `migrate` | Nhập dữ liệu từ runtime khác (hiện hỗ trợ OpenClaw) |
| `config` | Xuất schema cấu hình dạng máy đọc được |
| `completions` | Tạo script tự hoàn thành cho shell ra stdout |
| `hardware` | Phát hiện và kiểm tra phần cứng USB |
| `peripheral` | Cấu hình và nạp firmware thiết bị ngoại vi |

## Nhóm lệnh

### `onboard`

- `yantrikclaw onboard`
- `yantrikclaw onboard --channels-only`
- `yantrikclaw onboard --api-key <KEY> --provider <ID> --memory <sqlite|lucid|markdown|none>`
- `yantrikclaw onboard --api-key <KEY> --provider <ID> --model <MODEL_ID> --memory <sqlite|lucid|markdown|none>`

### `agent`

- `yantrikclaw agent`
- `yantrikclaw agent -m "Hello"`
- `yantrikclaw agent --provider <ID> --model <MODEL> --temperature <0.0-2.0>`
- `yantrikclaw agent --peripheral <board:path>`

### `gateway` / `daemon`

- `yantrikclaw gateway [--host <HOST>] [--port <PORT>]`
- `yantrikclaw daemon [--host <HOST>] [--port <PORT>]`

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

### `models`

- `yantrikclaw models refresh`
- `yantrikclaw models refresh --provider <ID>`
- `yantrikclaw models refresh --force`

`models refresh` hiện hỗ trợ làm mới danh mục trực tiếp cho các provider: `openrouter`, `openai`, `anthropic`, `groq`, `mistral`, `deepseek`, `xai`, `together-ai`, `gemini`, `ollama`, `astrai`, `venice`, `fireworks`, `cohere`, `moonshot`, `glm`, `zai`, `qwen` và `nvidia`.

### `channel`

- `yantrikclaw channel list`
- `yantrikclaw channel start`
- `yantrikclaw channel doctor`
- `yantrikclaw channel bind-telegram <IDENTITY>`
- `yantrikclaw channel add <type> <json>`
- `yantrikclaw channel remove <name>`

Lệnh trong chat khi runtime đang chạy (Telegram/Discord):

- `/models`
- `/models <provider>`
- `/model`
- `/model <model-id>`

Channel runtime cũng theo dõi `config.toml` và tự động áp dụng thay đổi cho:
- `default_provider`
- `default_model`
- `default_temperature`
- `api_key` / `api_url` (cho provider mặc định)
- `reliability.*` cài đặt retry của provider

`add/remove` hiện chuyển hướng về thiết lập có hướng dẫn / cấu hình thủ công (chưa hỗ trợ đầy đủ mutator khai báo).

### `integrations`

- `yantrikclaw integrations info <name>`

### `skills`

- `yantrikclaw skills list`
- `yantrikclaw skills install <source>`
- `yantrikclaw skills remove <name>`

`<source>` chấp nhận git remote (`https://...`, `http://...`, `ssh://...` và `git@host:owner/repo.git`) hoặc đường dẫn cục bộ.

Skill manifest (`SKILL.toml`) hỗ trợ `prompts` và `[[tools]]`; cả hai được đưa vào system prompt của agent khi chạy, giúp model có thể tuân theo hướng dẫn skill mà không cần đọc thủ công.

### `migrate`

- `yantrikclaw migrate openclaw [--source <path>] [--dry-run]`

### `config`

- `yantrikclaw config schema`

`config schema` xuất JSON Schema (draft 2020-12) cho toàn bộ hợp đồng `config.toml` ra stdout.

### `completions`

- `yantrikclaw completions bash`
- `yantrikclaw completions fish`
- `yantrikclaw completions zsh`
- `yantrikclaw completions powershell`
- `yantrikclaw completions elvish`

`completions` chỉ xuất ra stdout để script có thể được source trực tiếp mà không bị lẫn log/cảnh báo.

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

## Kiểm tra nhanh

Để xác minh nhanh tài liệu với binary hiện tại:

```bash
yantrikclaw --help
yantrikclaw <command> --help
```
