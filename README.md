# llmtop

Realtime memory, SoC, and model-residency monitor for Apple Silicon.

`llmtop` is a terminal tool for watching what local-LLM workloads are doing to
your Mac. It samples kernel VM counters, SoC utilization / power / thermals
(via `macmon`'s IOReport bindings), and the set of model files that are
currently mapped or served on the box. It works as an interactive TUI, a
headless logger (CSV / JSONL), or a one-shot JSON dump.

## What it shows

- **Usage pane** — four stacked bars (CPU, GPU, Memory, Power) with chip name,
  per-cluster frequency, temperature, and package wattage.
- **Memory pane** — wired / active / inactive / free / compressor, compress
  and pageout rates, swap-in / swap-out rates, pressure level, and the
  current `iogpu.wired_limit_mb` value.
- **Active Models pane** — every model the host has loaded, with size on
  disk, resident bytes, and residency percent. Sources include:
  - **Ollama** via `GET /api/ps`
  - **LM Studio** via `GET /api/v0/models` (falls back to `/v1/models`)
  - **omlx** via `GET /v1/models/status` (auto-probes ports 8000, 5741 if not set)
  - **Files** — mapped `.gguf` / `.ggml` / `.safetensors` / `.bin` files in
    matched processes, located via `proc_pidinfo` (FDs + VM regions)
  - **Cmdline** — model paths passed as process arguments

## Requirements

- Apple Silicon Mac (M1 or later). The binary is arm64-only; it talks to
  IOKit / IOReport and reads kernel VM counters through Mach calls.
- macOS 13+ recommended.
- For building from source: Rust 1.85+ (`edition = "2024"`) and the Xcode
  Command Line Tools (`xcode-select --install`) for the system framework
  linkage.

The release binary only links system-provided libraries (`IOKit`,
`libIOReport`, `CoreFoundation`, `libobjc`, `libiconv`, `libSystem`) — no
extra runtime dependencies.

## Build

```sh
cargo build --release
```

Produces:

- `target/release/llmtop` — the monitor
- `target/release/probe-fds`, `target/release/probe-raw` — diagnostic probes
  for the FD / VM-region scanning logic in `sample::ffi`

## Install locally

```sh
cargo install --path . --bin llmtop
```

…or copy the binary somewhere on your `PATH`:

```sh
install -m 0755 target/release/llmtop /usr/local/bin/llmtop
```

## Usage

Interactive TUI:

```sh
llmtop
```

Headless mode with rolling CSV / JSONL:

```sh
llmtop --no-tui --interval 2 --log run.csv --jsonl run.jsonl
```

Single JSON snapshot to stdout:

```sh
llmtop --once
```

Show only specific panes:

```sh
llmtop --pane usage --pane models
```

Filter processes by substring (overrides the built-in LLM-name list):

```sh
llmtop --match ollama --match llama-server
```

### Alerts

macOS notifications via `osascript`:

```sh
llmtop --alert-swap-mb 2048 \
       --alert-swap-rate 500 \
       --alert-pressure
```

Alerts share a 30-second debounce.

### TUI keys

| Key      | Action                                    |
| -------- | ----------------------------------------- |
| `c`      | Cycle bar themes (status / solid / cool / warm / llmtop / rainbow) |
| `q` / `Esc` | Quit                                   |
| `Ctrl-C` | Quit                                      |

### All flags

Run `llmtop --help` for the authoritative list. Highlights:

- `-i, --interval <secs>` — sample interval (default 1.0)
- `--proc-scan-interval <secs>` — full process rescan period (default 5.0).
  Between full scans only RSS of cached PIDs is refreshed.
- `--ollama-port <port>` (default 11434)
- `--lmstudio-port <port>` (default 1234, env `LMSTUDIO_PORT`)
- `--omlx-port <port>` — override; otherwise auto-probes 8000 then 5741

## Deploying to other Apple Silicon Macs

Because `llmtop` is a single statically-self-contained Mach-O against system
frameworks, deploying to another Apple Silicon Mac is just a binary copy:

```sh
# on the build host
cargo build --release

# transfer
scp target/release/llmtop <user>@<host>:/tmp/llmtop

# on the target host
sudo install -m 0755 /tmp/llmtop /usr/local/bin/llmtop
```

Things to know on a fresh target:

- **Arch must match.** The binary is arm64-only. Don't ship it to Intel Macs;
  build there with the same `cargo build --release` invocation instead.
- **macOS version.** Built on Darwin 25 / macOS 26-era SDK against system
  frameworks that have been stable since macOS 13. If you target an older
  release, rebuild on that release.
- **Gatekeeper.** Unsigned binaries downloaded via a browser get quarantined.
  When transferring with `scp` / `rsync` no quarantine attribute is set, so
  no extra step is needed. If you ship via download, strip it with
  `xattr -d com.apple.quarantine /usr/local/bin/llmtop`.
- **No entitlements required.** IOReport metrics work without `sudo` on
  recent macOS releases. Process / FD scanning sees what the invoking user
  can see — to monitor model files held open by another user's process,
  run `llmtop` with sufficient privileges.

For repeatable distribution, the typical patterns are:

1. **Tarball** — `tar czf llmtop-arm64-macos.tar.gz -C target/release llmtop`
   and copy that around. SHA-256 it if you care about integrity.
2. **Internal Homebrew tap** — drop the tarball on an internal HTTPS host and
   write a formula that does `bin.install "llmtop"`.
3. **Ansible / config-management** — same binary copy, idempotent via the
   `copy` / `file` modules. The binary has no runtime config — flags only.

## Output formats

### `--once` / JSONL

A single `SampleSet` per line. Top-level keys: `wall_ts`, `memory`, `soc`,
`models`. See `src/sample/mod.rs` for the full schema; everything except the
internal `ts: Instant` is serialized.

### `--log` (CSV)

One header line, then one row per sample. Columns are the most useful
scalars: VM page counts × page size, swap totals, pressure level, per-rate
counters, per-cluster CPU / GPU freq + util, full power breakdown, CPU /
GPU temps, and aggregate model size / residency. Column list is documented
in `src/log.rs`.

## Project layout

- `src/main.rs` — CLI entry, headless loop
- `src/cli.rs` — `clap` argument definitions
- `src/sample/` — collection layer
  - `memory.rs` — `host_statistics64`, swap, pressure, iogpu wired limit
  - `soc.rs` — `macmon`-backed CPU / GPU / power / thermal sampler
  - `ffi.rs` — `proc_pidinfo` / `proc_pidfdinfo` FFI for model-file detection
  - `api.rs` — Ollama / omlx / LM Studio HTTP clients
  - `models.rs` — merge + dedupe across API / file / cmdline sources
  - `process.rs` — `sysinfo`-backed process enumerator with fast/slow scans
- `src/tui/` — Ratatui panes, layout, themes, formatters
- `src/log.rs` — CSV + JSONL appenders
- `src/alert.rs` — debounced macOS notifications
- `src/bin/probe_*.rs` — small standalone diagnostics for the FFI layer

## License

Apache-2.0.
