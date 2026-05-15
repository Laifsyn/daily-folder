# Prints

Automated folder creation system for print jobs.
Generates a date-based directory structure and, under certain
conditions, creates an `printed` subfolder inside the day's directory.

## Installation

The binary is compiled as part of the workspace:

```bash
cargo build --release
```

## Quick Start

```bash
# Continuous mode (daemon) — keeps running indefinitely
./daifo

# Continuous mode with custom interval (every 60 seconds)
./daifo --interval 60

# Run once for today and exit (useful for cron / task scheduler)
./daifo --once

# Specific date (administrative, single run)
./daifo --date 2026-03-07

# Date range — administrative backfill
./daifo --start 2026-03-01 --end 2026-03-31

# Specify configuration file
./daifo --settings-file ./my_config.toml

# Override the root directory
./daifo --root-directory D:\jobs
```

## CLI Options

| Option             | Environment Variable      | Description                                                                                   |
| ------------------ | ------------------------- | --------------------------------------------------------------------------------------------- |
| `--root-directory` | `PRINTED_ROOT_DIRECTORY` | Root directory where the date structure is created. Overrides what is defined in the `.toml`. |
| `--settings-file`  | `PRINTED_SETTINGS_FILE`  | Path to the `.toml` configuration file. Default: `./.settings/daifo.toml`.                 |
| `--date`           | `PRINTED_DATE`           | **(Admin)** Single date to process (`YYYY-MM-DD`). The program exits after processing it.     |
| `--start`          | `PRINTED_START`          | **(Admin)** Start of backfill range (`YYYY-MM-DD`). Use with `--end`.                         |
| `--end`            | `PRINTED_END`            | **(Admin)** End of backfill range (`YYYY-MM-DD`). Use with `--start`.                         |
| `--once`           | `PRINTED_ONCE`           | Processes the current day once and exits. Useful for cron / scheduled tasks.                  |
| `--interval`       | `PRINTED_INTERVAL`       | Seconds between checks in continuous mode. Default: `180` (3 min).                            |

## Operation Modes

| Arguments           | Behavior                                                                                                     |
| ------------------- | ------------------------------------------------------------------------------------------------------------ |
| _(none)_            | **Continuous mode (daemon).** Processes the current day and repeats every `--interval` seconds indefinitely. |
| `--once`            | Processes the current day once and exits.                                                                    |
| `--date`            | Single run for the specified date, then exits.                                                               |
| `--start` + `--end` | Single run for the date range (administrative backfill), then exits.                                         |

## Configuration File

When running the program for the first time, `./.settings/daifo.toml`
is automatically created with default values and explanatory comments.

### Fields

```toml
# Root directory. Can be relative or absolute (e.g. "D:\")
root_directory = "./"

# Template with Chrono specifiers.
# %Y = 4-digit year   %m = 2-digit month
# %d = 2-digit day    %B = month name (from the [month_names] table)
create_path = "%Y/%m %B/%d"

# Extensions that trigger the "printed" folder
extension_trigger_printed = ["pdf", "png", "_tf"]

# If a day exceeds this number of files, "printed" is created
max_files_before_trigger = 5

# Name of the subfolder to create
printed_folder_name = "printed"
```

### Month Names Table

Optional. If the operating system handles localization well, it can be left
empty. By default it is populated with Spanish names for (hypothethical) compatibility with
Windows 7.
<!--I have no formal workflow to test if localization works properly in windows 7-->
```toml
[month_names]
"01" = "enero"
"02" = "febrero"
# ...
"12" = "diciembre"
```

## Domain Logic

1. Given a `root_directory` and a date, the program generates a path like:

   ```
   ./2025/03 marzo/15/
   ```

2. Checks whether the day's directory meets **at least one** of these conditions:
   - Contains more than `max_files_before_trigger` files.
   - Contains at least one file with an extension listed in
     `extension_trigger_printed`.

3. If met, creates the subfolder:

   ```
   ./2025/03 marzo/15/printed/
   ```

4. In **continuous mode**, this check repeats every `--interval` seconds.
   The operation is **idempotent**: if the `printed` folder already exists,
   it is neither duplicated nor modified.

## Logs

Logs are written to two destinations simultaneously:

- **Console** (`stderr`): filtered by the `RUST_LOG` environment variable
  (default: `daifo=info`).
- **File**: daily rolling at `./.logs/daifo/daifo.YYYY-MM-DD.log`.

To increase verbosity to debug level:

```bash
RUST_LOG=daifo=debug ./daifo
```

## Code Structure

```
src/daifo/
├── mod.rs        — root module, re-exports
├── error.rs      — DaifoError (static, thiserror)
├── settings.rs   — Settings, defaults, .toml loading/creation
├── template.rs   — expand_template (Chrono + month_names)
├── ops.rs        — should_create_printed, run_for_date, run_for_date_range
├── app.rs        — CLI, logging, spawn_blocking, continuous mode (application layer)
└── readme.md     — this file
```

The separation follows the principle that the **domain layer** knows nothing
about `tokio`, `color_eyre`, or `tracing` — it only handles static errors.
The **application layer** (`app.rs`) orchestrates blocking calls over
`spawn_blocking` and converts errors to `color_eyre::Report`.
