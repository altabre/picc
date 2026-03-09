# cc-connect

cc-connect 是一个 Telegram Bot，把 Claude Code 的对话会话桥接到 Telegram Forum Topics。

**每个 Topic = 独立的 Claude Code Session**，互不干扰。

---

## 架构

```
Telegram Topic A ──► Claude Session A (独立)
Telegram Topic B ──► Claude Session B (独立)
```

- 每个 `(chat_id, thread_id)` 对应独立的 Claude 会话
- 会话 ID 持久化到 SQLite，重启后自动恢复
- 使用 `claude -p --output-format stream-json --resume` 驱动

---

## 环境要求

| 依赖 | 说明 |
|------|------|
| Rust 1.75+ | `curl https://sh.rustup.rs -sSf \| sh` |
| Claude Code CLI | `npm install -g @anthropic-ai/claude-code` 并完成登录 |
| Telegram Bot Token | 通过 @BotFather 创建，**必须关闭 Group Privacy Mode** |

关闭 Privacy Mode 步骤：
BotFather → `/setprivacy` → 选择 Bot → `Disable`

---

## 快速部署

```bash
# 1. 克隆编译
git clone https://github.com/altabre/picc
cd picc
cargo build --release --bin cc-connect

# 2. 创建 .env
cat > .env << EOF
TELEGRAM_BOT_TOKEN=你的Token
APPROVED_DIRECTORY=/home/coder
ALLOWED_USERS=你的TelegramUserID
DB_PATH=cc_connect.db
RUST_LOG=info
EOF

# 3. 启动
./target/release/cc-connect

# 后台运行
nohup ./target/release/cc-connect > cc-connect.log 2>&1 &
```

获取 Telegram 用户 ID：向 @userinfobot 发送任意消息。

---

## Bot 命令

| 命令 | 说明 |
|------|------|
| 任意文字 | 发送给该 Topic 的 Claude 会话 |
| `/status` | 查看所有活跃 Topic 会话 |
| `/new` | 清除当前 Topic 会话，重新开始 |

---

## Agent 接入

通过 Telegram Bot API 向指定 Topic 发消息，Bot 自动维护该 Topic 的 Claude 会话：

```bash
curl -X POST "https://api.telegram.org/bot<TOKEN>/sendMessage" \
  -H "Content-Type: application/json" \
  -d '{
    "chat_id": -1001234567890,
    "message_thread_id": 2,
    "text": "你的消息"
  }'
```

Bot 会在同一 Topic 回复，保持会话隔离。

---

## 常见问题

| 问题 | 解决 |
|------|------|
| 收不到群消息 | BotFather → `/setprivacy` → Disable |
| `nested session` 错误 | 代码已自动 unset CLAUDECODE 环境变量 |
| `stream-json requires --verbose` | 代码已自动加 `--verbose` |
| 会话异常 | 发送 `/new` 重置当前 Topic 会话 |

---

## 项目结构

```
src/
├── bin/cc_connect.rs      # Bot 入口：HTTP long-polling + 消息路由
└── cc_connect/
    ├── config.rs          # .env 配置加载
    ├── store.rs           # SQLite：(chat_id, thread_id) → session_id
    ├── session.rs         # 调用 claude CLI，解析 stream-json
    └── formatter.rs       # Telegram 消息格式化
```
