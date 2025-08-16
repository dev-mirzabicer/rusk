# Rusk Task Manager - Setup and Deployment Guide

A comprehensive guide for setting up, deploying, and maintaining Rusk in various environments.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Installation Methods](#installation-methods)
3. [Configuration](#configuration)
4. [Development Setup](#development-setup)
5. [Deployment Options](#deployment-options)
6. [Database Management](#database-management)
7. [Troubleshooting](#troubleshooting)
8. [Maintenance](#maintenance)

## Quick Start

### Prerequisites

- **Operating System**: Linux, macOS, or Windows
- **Rust**: 1.75.0 or later (for building from source)
- **Storage**: Minimum 10MB for application + data storage needs

### Install from Source (Recommended)

```bash
# Clone the repository
git clone https://github.com/rusk-task-manager/rusk
cd rusk

# Build and install
cargo install --path crates/rusk-cli

# Verify installation
rusk --version
```

### First Run

```bash
# Create your first task
rusk add "Welcome to Rusk" --due tomorrow

# List tasks
rusk list

# Get help
rusk --help
```

## Installation Methods

### Method 1: Build from Source (Recommended)

**Advantages**: Latest features, optimal performance for your system
**Requirements**: Rust toolchain

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone and build
git clone https://github.com/rusk-task-manager/rusk
cd rusk
cargo build --release

# Install system-wide
cargo install --path crates/rusk-cli

# Or run directly
./target/release/rusk --help
```

### Method 2: Pre-built Binaries

**Advantages**: No compilation required
**Availability**: GitHub Releases

```bash
# Download latest release (replace with actual URL)
curl -L https://github.com/rusk-task-manager/rusk/releases/latest/download/rusk-linux-x86_64.tar.gz | tar xz

# Make executable and install
chmod +x rusk
sudo mv rusk /usr/local/bin/

# Verify
rusk --version
```

### Method 3: Container Deployment

**Advantages**: Isolated environment, easy deployment
**Requirements**: Docker or Podman

```dockerfile
# Example Dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y sqlite3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/rusk /usr/local/bin/
VOLUME ["/data"]
WORKDIR /data
ENTRYPOINT ["rusk"]
```

```bash
# Build and run
docker build -t rusk .
docker run -v $(pwd)/data:/data rusk list
```

## Configuration

### Configuration File

Rusk uses a TOML configuration file located at:
- **Linux/macOS**: `~/.config/rusk/config.toml`
- **Windows**: `%APPDATA%/rusk/config.toml`

Create the configuration directory and file:

```bash
# Linux/macOS
mkdir -p ~/.config/rusk
cat > ~/.config/rusk/config.toml << 'EOF'
# Rusk Configuration File

# Default filters for list command
default_filters = []

[recurrence]
# Default timezone for recurring tasks
default_timezone = "UTC"  # Change to your timezone, e.g., "America/New_York"

# How far ahead to create task instances (days)
lookahead_days = 30

# Minimum number of future instances to maintain
min_upcoming_instances = 1

# Maximum tasks to create in one batch
max_batch_size = 100

# Whether to create instances for missed past occurrences
enable_catchup = false

# Include recent past in materialization window (days)
materialization_grace_days = 3
EOF
```

### Environment Variables

Override configuration with environment variables:

```bash
# Database location
export RUSK_DATABASE_URL="$HOME/.local/share/rusk/tasks.db"

# Default timezone
export RUSK_DEFAULT_TIMEZONE="America/New_York"

# Enable debug logging
export RUST_LOG=debug

# Run with custom settings
rusk list
```

### Timezone Configuration

Set your local timezone for accurate recurring task scheduling:

```bash
# Find your timezone
rusk recur timezones --search "$(timedatectl show -p Timezone --value 2>/dev/null || echo 'america')"

# Update configuration
rusk config set recurrence.default_timezone "Your/Timezone"
```

## Development Setup

### Prerequisites

```bash
# Install Rust with recommended components
rustup install stable
rustup component add rustfmt clippy

# Install development tools
cargo install cargo-watch    # File watching
cargo install cargo-llvm-cov # Coverage analysis
cargo install sqlx-cli       # Database migrations
```

### Project Setup

```bash
# Clone and setup
git clone https://github.com/rusk-task-manager/rusk
cd rusk

# Install dependencies
cargo build

# Run tests
cargo test --workspace

# Check code formatting
cargo fmt --all -- --check

# Run lints
cargo clippy --workspace -- -D warnings
```

### Database Development

```bash
# Create development database
export DATABASE_URL="sqlite:dev.db"

# Run migrations
sqlx database create
sqlx migrate run

# Generate query metadata (for compile-time checking)
cargo sqlx prepare
```

### Running in Development

```bash
# Watch mode for rapid iteration
cargo watch -x "run -p rusk-cli -- list"

# With debug logging
RUST_LOG=debug cargo run -p rusk-cli -- add "Test task"

# Run specific tests
cargo test --package rusk-core test_name
```

## Deployment Options

### Single User (Desktop)

**Suitable for**: Personal use, single workstation
**Installation**: User-level binary in `~/.cargo/bin/`

```bash
# Install for current user
cargo install --path crates/rusk-cli

# Add to shell profile
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

### Multi-User (Server)

**Suitable for**: Shared systems, multiple users
**Installation**: System-wide with shared data

```bash
# Install system-wide
sudo cargo install --path crates/rusk-cli --root /usr/local

# Create shared data directory
sudo mkdir -p /var/lib/rusk
sudo chmod 755 /var/lib/rusk

# Configure for multi-user
sudo tee /etc/rusk.conf << 'EOF'
[recurrence]
default_timezone = "UTC"
lookahead_days = 30
EOF
```

### Container Deployment

**Suitable for**: Cloud deployment, microservices
**Benefits**: Isolation, easy scaling, consistent environment

```yaml
# docker-compose.yml
version: '3.8'
services:
  rusk:
    build: .
    volumes:
      - rusk_data:/data
      - ./config.toml:/data/config.toml:ro
    environment:
      - RUST_LOG=info
    restart: unless-stopped

volumes:
  rusk_data:
```

### Web API Deployment (Future)

**Note**: Web API is planned for future releases

```yaml
# Future web service deployment
version: '3.8'
services:
  rusk-api:
    image: rusk/api:latest
    ports:
      - "8080:8080"
    environment:
      - DATABASE_URL=sqlite:///data/rusk.db
      - RUST_LOG=info
    volumes:
      - rusk_data:/data
```

## Database Management

### Database Location

Default database locations:
- **Linux**: `~/.local/share/rusk/rusk.db`
- **macOS**: `~/Library/Application Support/rusk/rusk.db`
- **Windows**: `%APPDATA%/rusk/rusk.db`

### Backup and Restore

```bash
# Backup database
cp ~/.local/share/rusk/rusk.db ~/rusk-backup-$(date +%Y%m%d).db

# Restore from backup
cp ~/rusk-backup-20241215.db ~/.local/share/rusk/rusk.db

# Export to JSON (planned feature)
rusk export --format json > rusk-export.json

# Import from JSON (planned feature)
rusk import rusk-export.json
```

### Migration Management

```bash
# Check migration status
sqlx migrate info --database-url "sqlite:~/.local/share/rusk/rusk.db"

# Apply pending migrations
rusk migrate

# Rollback migrations (development only)
sqlx migrate revert --database-url "sqlite:dev.db"
```

### Database Maintenance

```bash
# Vacuum database (compact and optimize)
sqlite3 ~/.local/share/rusk/rusk.db "VACUUM;"

# Analyze statistics
sqlite3 ~/.local/share/rusk/rusk.db "ANALYZE;"

# Check integrity
sqlite3 ~/.local/share/rusk/rusk.db "PRAGMA integrity_check;"
```

## Troubleshooting

### Common Issues

#### Permission Errors

```bash
# Fix database permissions
chmod 644 ~/.local/share/rusk/rusk.db
chmod 755 ~/.local/share/rusk/

# Fix config permissions
chmod 644 ~/.config/rusk/config.toml
```

#### Database Corruption

```bash
# Check for corruption
sqlite3 ~/.local/share/rusk/rusk.db "PRAGMA integrity_check;"

# Repair minor corruption
sqlite3 ~/.local/share/rusk/rusk.db ".dump" | sqlite3 rusk_repaired.db
mv rusk_repaired.db ~/.local/share/rusk/rusk.db
```

#### Performance Issues

```bash
# Check database size
ls -lh ~/.local/share/rusk/rusk.db

# Optimize database
sqlite3 ~/.local/share/rusk/rusk.db "VACUUM; ANALYZE;"

# Reduce lookahead for large datasets
rusk config set recurrence.lookahead_days 14
```

#### Timezone Issues

```bash
# Verify system timezone
timedatectl show -p Timezone

# List available timezones
rusk recur timezones --search "your_region"

# Update configuration
rusk config set recurrence.default_timezone "Correct/Timezone"
```

### Debug Mode

Enable detailed logging for troubleshooting:

```bash
# Full debug output
RUST_LOG=debug rusk list

# Module-specific debugging
RUST_LOG=rusk_core::recurrence=debug rusk recur preview abc123

# Save debug output
RUST_LOG=debug rusk list 2> debug.log
```

### Performance Monitoring

```bash
# Time command execution
time rusk list "complex filter expression"

# Memory usage monitoring
/usr/bin/time -v rusk recur preview abc123 --count 100

# Database query analysis
sqlite3 ~/.local/share/rusk/rusk.db ".timer on" ".explain on" "SELECT * FROM tasks;"
```

## Maintenance

### Regular Maintenance Tasks

#### Weekly

```bash
# Backup database
cp ~/.local/share/rusk/rusk.db ~/backups/rusk-$(date +%Y%m%d).db

# Check for updates
cargo install --list | grep rusk-cli
```

#### Monthly

```bash
# Optimize database
sqlite3 ~/.local/share/rusk/rusk.db "VACUUM; ANALYZE;"

# Clean old backups (keep last 12)
find ~/backups -name "rusk-*.db" -type f | sort | head -n -12 | xargs rm -f

# Archive completed tasks (if desired)
rusk archive --before "3 months ago" --completed
```

#### As Needed

```bash
# Update to latest version
cargo install --path crates/rusk-cli --force

# Rebuild with optimizations
RUSTFLAGS="-C target-cpu=native" cargo install --path crates/rusk-cli --force

# Export data before major updates
rusk export --all > rusk-full-backup.json
```

### Monitoring and Health Checks

```bash
# Basic health check
rusk list --count 1 > /dev/null && echo "Rusk is healthy"

# Performance benchmark
time rusk list > /dev/null

# Storage usage
du -sh ~/.local/share/rusk/

# Configuration validation
rusk config validate
```

### Security Considerations

#### File Permissions

```bash
# Secure configuration
chmod 600 ~/.config/rusk/config.toml

# Secure database
chmod 600 ~/.local/share/rusk/rusk.db
```

#### Data Protection

```bash
# Encrypt backups (GPG)
gpg --cipher-algo AES256 --compress-algo 1 --symmetric \
    --output rusk-backup-encrypted.gpg \
    ~/.local/share/rusk/rusk.db

# Encrypted backup script
#!/bin/bash
DATE=$(date +%Y%m%d)
cp ~/.local/share/rusk/rusk.db /tmp/rusk-$DATE.db
gpg --symmetric --output ~/backups/rusk-$DATE.db.gpg /tmp/rusk-$DATE.db
rm /tmp/rusk-$DATE.db
```

## Advanced Configuration

### Custom Database Location

```bash
# Use custom database location
export RUSK_DATABASE_URL="/path/to/custom/location/tasks.db"
rusk list

# Make permanent
echo 'export RUSK_DATABASE_URL="/path/to/custom/location/tasks.db"' >> ~/.bashrc
```

### Shell Integration

#### Bash Completion

```bash
# Generate completion script
rusk completions bash > ~/.local/share/bash-completion/completions/rusk

# Add to bashrc
echo 'source ~/.local/share/bash-completion/completions/rusk' >> ~/.bashrc
```

#### Aliases and Functions

```bash
# Useful aliases
alias t='rusk'
alias ta='rusk add'
alias tl='rusk list'
alias td='rusk do'

# Quick task addition function
qt() {
    rusk add "$*" --due today
}

# Project-specific task listing
tp() {
    rusk list "project:$1"
}
```

### Integration Scripts

#### Daily Task Summary

```bash
#!/bin/bash
# daily-summary.sh

echo "=== Rusk Daily Summary ==="
echo "Overdue tasks:"
rusk list overdue

echo -e "\nDue today:"
rusk list due:today

echo -e "\nDue tomorrow:"
rusk list due:tomorrow

echo -e "\nTask statistics:"
echo "Total pending: $(rusk list status:pending --count)"
echo "Total completed today: $(rusk list 'status:completed and completed:today' --count)"
```

#### Recurring Task Health Check

```bash
#!/bin/bash
# recur-health.sh

echo "=== Recurring Task Health Check ==="

# List all recurring series
rusk list has:recurrence --format=ids | while read -r task_id; do
    echo "Checking series: $task_id"
    rusk recur stats "$task_id" --brief
done
```

## Conclusion

This setup guide provides comprehensive instructions for deploying and maintaining Rusk in various environments. Choose the deployment method that best fits your needs:

- **Personal use**: Single-user installation with user-level permissions
- **Team use**: Multi-user installation with shared configuration
- **Production**: Container deployment with proper backup strategies

For additional help:
- Check the built-in help: `rusk --help`
- Visit the troubleshooting section above
- Review the [Recurrence Guide](RECURRENCE_GUIDE.md) for advanced features
- Report issues on the project repository

Remember to regularly backup your data and keep your installation updated for the best experience.