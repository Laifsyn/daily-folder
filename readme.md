# daifo — Daily Folder Organizer

Creates a dated folder structure (e.g. `2025/03 marzo/15`) each day,
optionally places a symbolic link so FTP users can jump straight to today's
folder, and auto-creates a "printed" sub-folder when enough files
accumulate or a trigger extension (`.pdf`, `.png`) appears.

## Quick start

```sh
# Create config and run once
daifo --once
# Edit .setting/daifo.toml to match your paths, then…
daifo                   # continuous mode (daemon)
```

## Modes

| Command                     | What it does                     |
| --------------------------- | -------------------------------- |
| `daifo`                     | Runs forever, checks every 180 s |
| `daifo --once`              | Processes today and exits        |
| `daifo --date 2025-03-15`   | Processes a single date          |
| `daifo --start .. --end ..` | Back-fills a date range          |

## Configuration (`.setting/daifo.toml`)

```toml
root_directory = "D:/ftp"
create_path = "./%Y/%m %B/%d"
extension_trigger_printed = ["pdf", "png"]
max_files_before_trigger = 5
printed_folder_name = "printed"
create_link_to_daily_folder = true
duplicate_daily_folder_link_to = ["C:\\Users\\Public\\Desktop"]

[month_names]
01 = "enero"
02 = "febrero"
...
```

## Important notes

- **Symlinks** require admin rights (or Developer Mode). Without them
  the program skips symlink creation and continues normally.
- **Stale links** are cleaned up automatically — no manual work needed.
- Set up a **scheduled task** (highest privileges, _At log on_) to start
  on boot. A sample XML export is included in the repo. Worth the note that
  it assumes the file was installed in `C:\bins\daifo`, so changes have to be
  made accordingly.
- Logs rotate weekly in `.logs/daifo/`.
- Tray icon (right-click) → _Show/Hide console_ / _Exit_.
- Month names default to Spanish; change them in the `[month_names]` table.
