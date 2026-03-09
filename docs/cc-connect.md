# cc-connect

Bridge Claude Code sessions to Telegram forum topics. Each topic = independent Claude session.

```
Topic A  ──►  Claude Session A
Topic B  ──►  Claude Session B  (isolated, no shared context)
```

## Install

```bash
curl -sSf https://raw.githubusercontent.com/altabre/picc/dev/custom-modules/install-cc-connect.sh | bash
```

Or manually:

```bash
git clone -b dev/custom-modules https://github.com/altabre/picc
cd picc && cargo build --release --bin cc-connect
cp target/release/cc-connect ~/.local/bin/
```

## Config

Edit `~/.cc-connect/config.toml` (created automatically on first install):

```toml
[bot]
telegram_bot_token = "123456:ABC-DEF..."   # @BotFather
approved_directory = "/home/coder"          # Claude working directory
allowed_users = [123456789]                 # Your Telegram ID (@userinfobot)
allowed_topics = [                          # Leave empty = all topics
    "https://t.me/3816634435/84",
    "https://t.me/3816634435/2",
]

[db]
path = "~/.cc-connect/sessions.db"
```

Or use `.env`:

```env
TELEGRAM_BOT_TOKEN=123456:ABC-DEF...
APPROVED_DIRECTORY=/home/coder
ALLOWED_USERS=123456789
ALLOWED_TOPICS=84,2          # thread IDs from topic URLs
```

## Run

```bash
cc-connect
```

## Setup (one-time)

**1. Create a Telegram bot** via [@BotFather](https://t.me/BotFather), copy the token.

**2. Disable Group Privacy Mode** (required to read all group messages):
```
BotFather → /setprivacy → [select your bot] → Disable
```

**3. Add bot to your Telegram group** with forum topics enabled.

**4. Get your user ID** by messaging [@userinfobot](https://t.me/userinfobot).

## Commands

| Command | Description |
|---------|-------------|
| Any text | Forwarded to that topic's Claude session |
| `/new` | Start a fresh Claude session in this topic |
| `/status` | List all active topic sessions |

## How it works

- Polls Telegram via `getUpdates` (no public IP needed)
- Each topic's session persists in SQLite — survives restarts
- Runs `claude -p "msg" --output-format stream-json --resume <session_id>`
- Unsets `CLAUDECODE` env var to allow nested invocation

## Requirements

- [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code) installed and authenticated
- Rust 1.75+ (only for building from source)

