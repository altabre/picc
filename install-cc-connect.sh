#!/usr/bin/env bash
set -e

REPO="https://github.com/altabre/picc"
BRANCH="dev/custom-modules"

echo "Installing cc-connect..."

# Check dependencies
if ! command -v cargo &>/dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

if ! command -v claude &>/dev/null; then
    echo "Error: Claude Code CLI not found. Install it first:"
    echo "  npm install -g @anthropic-ai/claude-code && claude login"
    exit 1
fi

# Clone or update
if [ -d "$HOME/.cc-connect/src" ]; then
    echo "Updating..."
    git -C "$HOME/.cc-connect/src" pull
else
    echo "Cloning..."
    git clone -b "$BRANCH" --depth=1 "$REPO" "$HOME/.cc-connect/src"
fi

# Build
echo "Building..."
cd "$HOME/.cc-connect/src"
cargo build --release --bin cc-connect

# Install to PATH
mkdir -p "$HOME/.local/bin"
cp target/release/cc-connect "$HOME/.local/bin/cc-connect"

# Create default config if not exists
if [ ! -f "$HOME/.cc-connect/config.toml" ]; then
    mkdir -p "$HOME/.cc-connect"
    cat > "$HOME/.cc-connect/config.toml" << 'TOML'
[bot]
telegram_bot_token = ""   # From @BotFather
approved_directory = "/home/coder"
allowed_users = []        # Your Telegram user ID (get from @userinfobot)
allowed_topics = []       # e.g. ["https://t.me/xxx/84"] — empty = all topics

[db]
path = "~/.cc-connect/sessions.db"
TOML
    echo ""
    echo "Config created at: ~/.cc-connect/config.toml"
    echo "Edit it and add your TELEGRAM_BOT_TOKEN."
fi

echo ""
echo "Done! Run: cc-connect"
echo "Config:   ~/.cc-connect/config.toml"
