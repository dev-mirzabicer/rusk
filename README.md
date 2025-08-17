# 🚀 Rusk Task Manager

> A feature-rich, high-quality, robust CLI task management tool with advanced recurring task support

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](https://github.com/rusk-task-manager/rusk)

Rusk is a modern, powerful CLI task manager built in Rust that rivals commercial solutions like Todoist. It features sophisticated recurring task management, timezone awareness, natural language processing, and an intuitive command-line interface.

## ✨ Features

### Core Task Management
- 📝 **Rich Task Creation**: Natural language due dates, priorities, projects, tags, and dependencies
- 🔍 **Advanced Filtering**: Powerful query system with logical operators (`and`, `or`, `not`)
- 📊 **Project Organization**: Group related tasks for better workflow management
- 🔗 **Task Dependencies**: Block tasks until prerequisites are completed
- 📂 **Subtask Support**: Create hierarchical task structures

### Advanced Recurring Tasks
- 🔄 **Series-Based Recurrence**: Industry-standard approach matching calendar applications
- 🌍 **Timezone Awareness**: Full IANA timezone support with automatic DST handling
- 📅 **Flexible Patterns**: From simple daily tasks to complex business rules
- ⚡ **Smart Scheduling**: Human-friendly shortcuts (`--every weekdays --at "9am"`)
- 🎯 **Exception Handling**: Skip, move, or override individual occurrences
- 📈 **Series Management**: Pause, resume, duplicate, and analyze recurring series

### User Experience
- 🎨 **Beautiful Output**: Colored tables, visual indicators, and clear formatting
- 💬 **Natural Language**: Parse dates like "tomorrow", "next friday", "in 2 weeks"
- 🔧 **Smart Defaults**: Sensible defaults with full customization options
- 📚 **Comprehensive Help**: Detailed help text and examples for every command
- 🚀 **Performance**: Lightning-fast SQLite backend with optimized queries

## 🎯 Quick Start

### Installation

```bash
# Install from source (recommended)
git clone https://github.com/rusk-task-manager/rusk
cd rusk
cargo install --path crates/rusk-cli

# Verify installation
rusk --version
```

### Your First Tasks

```bash
# Simple task
rusk add "Buy groceries" --due tomorrow --project Personal

# Recurring task
rusk add "Daily standup" --every weekdays --at "9:00 AM" --project Work

# Complex task with dependencies
rusk add "Deploy website" --due "next friday" --depends-on abc123 --priority high

# List tasks
rusk list                     # Default view
rusk list due:today          # Tasks due today
rusk list project:Work       # Work-related tasks
```

## 🔄 Recurring Tasks Made Simple

### Human-Friendly Patterns

```bash
# Daily tasks
rusk add "Exercise" --every daily --at "6:00 AM"
rusk add "Check email" --every weekdays --at "9:00 AM"

# Weekly tasks  
rusk add "Team meeting" --every weekly --on monday --at "10:00 AM"
rusk add "Grocery shopping" --every weekly --on "saturday" --at "2:00 PM"

# Monthly tasks
rusk add "Pay rent" --every monthly --on 1 --at "9:00 AM"
rusk add "Monthly report" --every monthly --on "last friday"

# Limited recurrence
rusk add "Training sessions" --every weekly --count 8 --until "2025-12-31"
```

### Advanced Management

```bash
# Preview upcoming occurrences
rusk recur preview abc123 --count 10

# Skip specific occurrences
rusk recur skip abc123 --on "next friday"

# Move an occurrence
rusk recur move abc123 --from "2025-08-20 09:00" --to "2025-08-21 14:00"

# Pause/resume series
rusk recur pause abc123
rusk recur resume abc123

# Get detailed statistics
rusk recur stats abc123 --detailed
```

## 🌍 Timezone Support

Rusk handles timezones intelligently for accurate scheduling across regions:

```bash
# Set timezone for recurring tasks
rusk add "Daily sync" --every daily --at "3:00 PM" --timezone "Europe/London"

# Browse available timezones
rusk recur timezones --search "america" --common

# Update existing series timezone
rusk edit abc123 --timezone "Asia/Tokyo" --scope series
```

## 🔍 Advanced Filtering

Powerful query system for finding exactly what you need:

```bash
# Basic filters
rusk list status:pending
rusk list project:Work
rusk list tag:urgent

# Logical combinations
rusk list "status:pending and (project:Work or tag:urgent)"
rusk list "due:before:friday and not status:completed"

# Date-based filters
rusk list due:today
rusk list overdue
rusk list "due:after:monday and due:before:friday"

# Recurring task filters
rusk list has:recurrence
rusk list "has:recurrence and project:Work"
```

## 🎛️ Configuration

Customize Rusk to fit your workflow:

```toml
# ~/.config/rusk/config.toml

# Default filters for the list command
default_filters = []

[recurrence]
# Your local timezone
default_timezone = "America/New_York"

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
```

## 📖 Documentation

- **[Setup Guide](SETUP.md)**: Comprehensive installation and deployment instructions
- **[Recurrence Guide](RECURRENCE_GUIDE.md)**: Master recurring tasks with detailed examples
- **Built-in Help**: Use `rusk --help` or `rusk <command> --help` for detailed usage

## 🏗️ Architecture

Rusk is built with a modular, scalable architecture:

```
┌─────────────────────────────────────────────────────────────┐
│                     CLI Layer (rusk-cli)                    │
├─────────────────────────────────────────────────────────────┤
│ • Command Parsing           • Scope Resolution              │
│ • User Interaction          • Error Presentation            │
│ • Timezone Input/Output     • Progress Feedback             │
└─────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────┐
│                 Business Logic (rusk-core)                  │
├─────────────────────────────────────────────────────────────┤
│ • Repository Trait          • RecurrenceManager             │
│ • Series Operations         • MaterializationManager        │
│ • Exception Handling        • Timezone Utilities            │
│ • Transaction Coordination  • Configuration Management      │
└─────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────┐
│                Data Layer (SQLite + sqlx)                   │
├─────────────────────────────────────────────────────────────┤
│ • tasks                     • task_series                   │
│ • series_exceptions         • task_tags                     │
│ • task_dependencies         • projects                      │
└─────────────────────────────────────────────────────────────┘
```

### Key Components

- **Series-Based Recurrence**: Industry-standard approach for complex recurring patterns
- **Intelligent Materialization**: Just-in-time task instance creation with configurable policies
- **Exception Management**: Flexible handling of deviations from recurring patterns
- **Timezone Engine**: Full IANA timezone support with DST awareness
- **Performance Optimization**: Memory-efficient algorithms with compile-time SQL checking

## 🛠️ Development

### Building from Source

```bash
# Prerequisites
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup component add rustfmt clippy

# Clone and build
git clone https://github.com/rusk-task-manager/rusk
cd rusk
cargo build --release

# Run tests
cargo test --workspace

# Install locally
cargo install --path crates/rusk-cli
```

### Project Structure

```
rusk/
├── crates/
│   ├── rusk-core/          # Core business logic and data layer
│   └── rusk-cli/           # Command-line interface
├── migrations/             # Database schema migrations
├── docs/                   # Additional documentation
├── SETUP.md               # Deployment and configuration guide
├── RECURRENCE_GUIDE.md    # Detailed recurrence feature guide
└── README.md              # This file
```

### Contributing

We welcome contributions! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Write tests for your changes
4. Ensure all tests pass (`cargo test --workspace`)
5. Check formatting (`cargo fmt --all -- --check`)
6. Run lints (`cargo clippy --workspace -- -D warnings`)
7. Commit your changes (`git commit -m 'Add amazing feature'`)
8. Push to the branch (`git push origin feature/amazing-feature`)
9. Open a Pull Request

## 🚀 Performance

Rusk is designed for speed and efficiency:

- **Startup Time**: Sub-100ms for most commands
- **Memory Usage**: Minimal footprint with bounded resource usage
- **Database**: Optimized SQLite with proper indexing and query patterns
- **Scalability**: Handles thousands of tasks and complex recurring patterns efficiently

### Benchmarks

```bash
# Typical performance on modern hardware
Command                     Time
rusk list                  ~50ms
rusk add "new task"        ~30ms
rusk recur preview id      ~80ms
Complex filter query       ~120ms
```

## 🎯 Use Cases

### Personal Productivity
- **Daily routines**: Exercise, medication, habits
- **Weekly tasks**: Grocery shopping, cleaning, planning
- **Monthly responsibilities**: Bills, reports, reviews
- **Project management**: Break down large goals into manageable tasks

### Professional Workflows
- **Team coordination**: Regular meetings, check-ins, standups
- **Business processes**: Monthly reports, quarterly reviews
- **Development cycles**: Sprint planning, code reviews, deployments
- **Client management**: Regular follow-ups, milestone tracking

### Academic and Research
- **Study schedules**: Regular review sessions, assignment deadlines
- **Research tasks**: Data collection, analysis, writing schedules
- **Administrative duties**: Grade submission, committee meetings
- **Conference planning**: Abstract deadlines, presentation prep

## 🤝 Community

- **GitHub Discussions**: Share ideas and get help from the community
- **Issue Tracker**: Report bugs and request features
- **Contributing Guide**: Learn how to contribute to the project
- **Code of Conduct**: Our commitment to a welcoming community

## 📄 License

This project is licensed under 
 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

## 🙏 Acknowledgments

- **Rust Community**: For providing excellent tooling and ecosystem
- **SQLite**: For the robust, lightweight database engine
- **Clap**: For the powerful command-line parsing capabilities
- **Chrono**: For comprehensive date and time handling
- **RRULE Library**: For RFC 5545 recurrence rule support

---

<div align="center">

**Built with ❤️ in Rust**

*Rusk - Because your tasks deserve better management*

[⭐ Star on GitHub](https://github.com/rusk-task-manager/rusk) • [📖 Documentation](SETUP.md) • [🐛 Report Bug](https://github.com/rusk-task-manager/rusk/issues) • [💡 Request Feature](https://github.com/rusk-task-manager/rusk/issues)

</div>
