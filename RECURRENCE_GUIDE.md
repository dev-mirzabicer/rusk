# Rusk Recurrence Guide

A comprehensive guide to using Rusk's advanced recurring task features.

## Table of Contents

1. [Introduction](#introduction)
2. [Quick Start](#quick-start)
3. [Creating Recurring Tasks](#creating-recurring-tasks)
4. [Managing Series](#managing-series)
5. [Exception Handling](#exception-handling)
6. [Timezone Support](#timezone-support)
7. [Advanced Features](#advanced-features)
8. [Troubleshooting](#troubleshooting)

## Introduction

Rusk provides industry-leading recurring task management with features that rival commercial task management tools. Our series-based approach allows for sophisticated recurrence patterns while maintaining simplicity for everyday use.

### Key Concepts

- **Series**: A recurring task template that generates instances
- **Instance**: Individual occurrences of a recurring task
- **Template**: The base task that defines the series properties
- **Exception**: Modifications to specific occurrences (skip, move, override)
- **Materialization**: The process of creating task instances from series

## Quick Start

### Create Your First Recurring Task

```bash
# Simple daily task
rusk add "Take vitamins" --every daily --at "8:00 AM"

# Weekly team meeting
rusk add "Team standup" --every weekly --on mon --at "9:00 AM" --project Work

# Monthly report due
rusk add "Monthly report" --every monthly --on 1 --due "last day of month"
```

### View Upcoming Occurrences

```bash
# See next 10 occurrences
rusk recur preview abc123

# Show series information
rusk recur info abc123

# List all recurring tasks
rusk list has:recurrence
```

## Creating Recurring Tasks

### Using Human-Friendly Shortcuts

Rusk provides simple shortcuts for common patterns:

```bash
# Daily tasks
rusk add "Exercise" --every daily --at "6:00 AM"
rusk add "Check email" --every weekdays --at "9:00 AM"
rusk add "Sleep in" --every weekends --at "9:00 AM"

# Weekly tasks
rusk add "Grocery shopping" --every weekly --on sun --at "2:00 PM"
rusk add "Team meeting" --every weekly --on "mon,wed,fri" --at "10:00 AM"

# Monthly tasks
rusk add "Pay rent" --every monthly --on 1 --at "9:00 AM"
rusk add "Review finances" --every monthly --on "last friday"

# Yearly tasks
rusk add "Annual review" --every yearly --on "jan 15" --at "2:00 PM"
```

### Time Specifications

Rusk supports flexible time formats:

```bash
# 12-hour format
--at "9:00 AM"
--at "2:30 PM"
--at "11:59 PM"

# 24-hour format
--at "09:00"
--at "14:30"
--at "23:59"

# Natural language
--at "9am"
--at "2:30pm"
--at "noon"
--at "midnight"
```

### Date Specifications

For weekly and monthly recurrence:

```bash
# Days of week (multiple ways)
--on "mon"
--on "monday"
--on "mon,wed,fri"
--on "weekdays"
--on "weekends"

# Specific dates for monthly
--on 1                    # 1st of month
--on 15                   # 15th of month
--on "last day"           # Last day of month
--on "last friday"        # Last Friday of month
--on "first monday"       # First Monday of month
```

### Limiting Recurrence

You can limit how long a task recurs:

```bash
# End on specific date
rusk add "Project meetings" --every weekly --until "2025-12-31"

# Maximum number of occurrences
rusk add "Training sessions" --every weekly --count 8

# Both (whichever comes first)
rusk add "Limited task" --every daily --until "2025-12-31" --count 100
```

### Timezone Support

Specify timezone for accurate scheduling:

```bash
# Using IANA timezone names
rusk add "Call client" --every weekly --at "9:00 AM" --timezone "America/New_York"
rusk add "Team sync" --every daily --at "3:00 PM" --timezone "Europe/London"

# System will handle DST transitions automatically
rusk add "Morning routine" --every daily --at "7:00 AM" --timezone "America/Los_Angeles"
```

### Advanced RRULE Patterns

For complex patterns, use raw RRULE:

```bash
# Every other week on Monday and Wednesday
rusk add "Bi-weekly meetings" --recurrence "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE"

# Last Friday of every quarter
rusk add "Quarterly review" --recurrence "FREQ=MONTHLY;INTERVAL=3;BYDAY=-1FR"

# Every weekday except holidays (with exceptions)
rusk add "Daily standup" --recurrence "FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR"
```

## Managing Series

### Viewing Series Information

```bash
# Comprehensive series details
rusk recur info abc123

# Preview upcoming occurrences
rusk recur preview abc123 --count 20

# Show all exceptions
rusk recur exceptions abc123

# Detailed statistics
rusk recur stats abc123 --detailed
```

### Modifying Series

When editing recurring tasks, you'll be prompted for scope:

```bash
# Edit with interactive scope selection
rusk edit abc123 --name "Updated task name"

# Force specific scope (no prompts)
rusk edit abc123 --scope series --due "10:00 AM"
rusk edit abc123 --scope future --project "New Project"
rusk edit abc123 --scope occurrence --priority high
```

#### Scope Options

- **This occurrence only**: Modify just this instance
- **This and future**: Update the series from this point forward
- **Entire series**: Modify all past and future occurrences

### Series Lifecycle

```bash
# Pause series (stop generating new instances)
rusk recur pause abc123

# Resume paused series
rusk recur resume abc123

# Archive completed series
rusk recur archive abc123

# Duplicate series with modifications
rusk recur duplicate abc123 "New task name" --timezone "Europe/Paris"
```

## Exception Handling

Exceptions allow you to modify specific occurrences without affecting the entire series.

### Skipping Occurrences

```bash
# Skip a specific date
rusk recur skip abc123 --on "2025-08-20"

# Skip using natural language
rusk recur skip abc123 --on "next friday"
rusk recur skip abc123 --on "tomorrow"

# Bulk skip multiple dates
rusk recur bulk-skip abc123 --dates "2025-08-20,2025-08-27,2025-09-03"

# Skip date range
rusk recur bulk-skip abc123 --from "2025-08-20" --to "2025-08-30"
```

### Moving Occurrences

```bash
# Move to different time
rusk recur move abc123 --from "2025-08-20 09:00" --to "2025-08-20 14:00"

# Move to different day
rusk recur move abc123 --from "2025-08-20" --to "2025-08-22"

# Move with natural language
rusk recur move abc123 --from "next tuesday" --to "next wednesday 2pm"
```

### Managing Exceptions

```bash
# View all exceptions for a series
rusk recur exceptions abc123

# Remove specific exceptions
rusk recur remove-exceptions abc123 --dates "2025-08-20,2025-08-27"

# Remove all exceptions (restore original schedule)
rusk recur remove-exceptions abc123 --all
```

## Timezone Support

### Available Timezones

```bash
# List common timezones
rusk recur timezones --common

# Search for specific timezone
rusk recur timezones --search "america"
rusk recur timezones --search "new_york"

# Detailed timezone information
rusk recur timezones --search "london" --detailed
```

### Timezone Best Practices

1. **Always specify timezone** for recurring tasks to avoid confusion
2. **Use IANA names** like "America/New_York" instead of "EST"
3. **Consider DST** when scheduling - Rusk handles transitions automatically
4. **Update timezone** when moving or traveling:

```bash
# Update series timezone
rusk edit abc123 --timezone "Europe/London" --scope series
```

### DST Handling

Rusk automatically handles Daylight Saving Time transitions:

- **Spring forward**: Tasks scheduled during the "lost hour" are moved to the next valid time
- **Fall back**: Tasks maintain their local time meaning
- **Time zone changes**: When updating timezone, existing instances keep their UTC time

## Advanced Features

### Complex Recurrence Patterns

```bash
# Every 3 months on the 15th
rusk add "Quarterly check" --recurrence "FREQ=MONTHLY;INTERVAL=3;BYMONTHDAY=15"

# Every Tuesday and Thursday
rusk add "Gym sessions" --recurrence "FREQ=WEEKLY;BYDAY=TU,TH"

# First Monday of every month
rusk add "Monthly meeting" --recurrence "FREQ=MONTHLY;BYDAY=1MO"

# Last working day of month
rusk add "Month-end report" --recurrence "FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1"
```

### Series Statistics

Monitor series health and completion rates:

```bash
# Basic statistics
rusk recur stats abc123

# Detailed breakdown
rusk recur stats abc123 --detailed
```

Statistics include:
- Total occurrences created
- Completion rate
- Average completion time
- Exception count by type
- Series health score

### Filtering Recurring Tasks

```bash
# Show all recurring tasks
rusk list has:recurrence

# Show non-recurring tasks only
rusk list no:recurrence

# Show paused series
rusk list "has:recurrence and status:paused"

# Show overdue recurring tasks
rusk list "has:recurrence and overdue"
```

### Integration with Projects and Tags

```bash
# Recurring task with project and tags
rusk add "Code review" \
  --every weekdays \
  --at "10:00 AM" \
  --project Development \
  --tag review --tag daily \
  --timezone "America/New_York"

# Filter by project and recurrence
rusk list "project:Development and has:recurrence"
```

## Troubleshooting

### Common Issues

**Q: My recurring task isn't generating new instances**
A: Check if the series is paused with `rusk recur info <id>`. Resume with `rusk recur resume <id>`.

**Q: Times are showing in wrong timezone**
A: Verify the series timezone with `rusk recur info <id>`. Update with `rusk edit <id> --timezone "Your/Timezone" --scope series`.

**Q: Too many instances created**
A: Check the materialization window in your config. Reduce `lookahead_days` if needed.

**Q: Skipped occurrence still showing**
A: Ensure you used the correct date format. Check exceptions with `rusk recur exceptions <id>`.

### Error Messages

Rusk provides detailed error messages with suggestions:

```bash
# Invalid timezone
Error: Unknown timezone 'Invalid/Timezone'
Tip: Use standard IANA timezone names like 'America/New_York'
Tip: Run 'rusk recur timezones' to see common timezones

# Invalid RRULE
Error: Invalid recurrence rule: "FREQ=INVALID"
Tip: Use shortcuts like --every daily or valid RRULE syntax
Example: --every weekdays --at '9:00 AM'
```

### Getting Help

```bash
# Command-specific help
rusk add --help
rusk recur skip --help

# List all recurrence commands
rusk recur --help

# View series information for debugging
rusk recur info <id>
```

### Performance Tips

1. **Use appropriate lookahead**: Don't set `lookahead_days` too high
2. **Clean up old data**: Archive completed series periodically
3. **Limit preview counts**: Use reasonable values for `--count` in preview
4. **Batch operations**: Use bulk commands for multiple operations

## Advanced Examples

### Complex Business Scenarios

```bash
# Bi-weekly sprint planning (every other Monday)
rusk add "Sprint planning" \
  --recurrence "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO" \
  --at "9:00 AM" \
  --project Development \
  --timezone "America/New_York"

# Monthly reports due on last business day
rusk add "Monthly report" \
  --recurrence "FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1" \
  --at "5:00 PM" \
  --project Finance

# Quarterly reviews on first Monday of Jan, Apr, Jul, Oct
rusk add "Quarterly review" \
  --recurrence "FREQ=YEARLY;BYMONTH=1,4,7,10;BYDAY=1MO" \
  --at "2:00 PM" \
  --project Management
```

### Personal Use Cases

```bash
# Workout schedule (Mon, Wed, Fri)
rusk add "Workout" \
  --every weekly \
  --on "mon,wed,fri" \
  --at "6:00 AM" \
  --tag fitness --tag health

# Bill reminders (various dates)
rusk add "Electric bill" --every monthly --on 15 --tag bills
rusk add "Rent payment" --every monthly --on 1 --tag bills --priority high

# Seasonal tasks
rusk add "Change air filter" \
  --recurrence "FREQ=MONTHLY;INTERVAL=3" \
  --tag maintenance --tag home
```

### Migration from Other Tools

When migrating from other task managers:

1. **Identify recurring patterns** in your current tool
2. **Convert to Rusk syntax** using shortcuts or RRULE
3. **Set appropriate timezones** for location-sensitive tasks
4. **Test with preview** before committing to complex patterns
5. **Use exceptions** to handle one-off modifications

## Conclusion

Rusk's recurrence system provides the flexibility and power needed for sophisticated task management while remaining approachable for everyday use. The combination of human-friendly shortcuts and advanced RRULE support ensures you can handle any recurring task pattern your workflow requires.

For additional help, use the built-in command help (`--help`) or explore the examples provided throughout this guide.