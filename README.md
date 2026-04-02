# kubeview

A fast terminal UI for Kubernetes. Browse pods, services, deployments, ingresses, and secrets — tail logs, exec into containers, port-forward, and manage your cluster without leaving the terminal.

Inspired by [k9s](https://k9scli.io) and [holo](https://github.com/measure-sh/holo).

![Rust](https://img.shields.io/badge/rust-stable-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## Features

- **6 tabs** — Pods, Services, Deployments, Ingresses, Secrets, Events
- **Pod logs** — live-tailing with `l`, colorized by log level
- **Exec** — drop into a container shell with `e` (tries bash, falls back to sh)
- **Port-forward** — `p` to start, `P` to view/stop active forwards
- **Describe** — full `kubectl describe`-style output for any resource
- **YAML view** — raw YAML for any resource with `y`
- **Secrets** — decoded view with `Enter`, raw base64 YAML with `y`, edit values with `e`
- **Scale deployments** — `s` to set replica count
- **Rollout restart** — `r` on Deployments tab with confirmation dialog
- **Fuzzy search** — filter pods instantly with `/`
- **Namespace cycling** — `n` to rotate through all namespaces
- **TSH cluster picker** — `c` to switch Teleport clusters (calls `tsh kube login`)
- **Themes** — `T` to cycle Default / Dracula / Nord / Tokyo Night
- **Prod safety** — context name turns red when it contains "prod"

## Install

### Homebrew (macOS / Linux)

```bash
brew tap harshjain-dev/kubeview https://github.com/harshjain-dev/kubeview
brew install kubeview
```

### Cargo

```bash
cargo install kubeview
```

### Build from source

```bash
git clone https://github.com/harshjain-dev/kubeview
cd kubeview
cargo build --release
# binary is at ./target/release/kubeview
sudo cp target/release/kubeview /usr/local/bin/
```

## Usage

```bash
kubeview
```

Requires a valid `~/.kube/config` (same as `kubectl`).

## Keybindings

### Global

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Next / prev tab |
| `1`–`6` | Jump to tab directly |
| `j` / `k` / `↑↓` | Navigate up / down |
| `g` / `G` | Jump to top / bottom |
| `n` | Cycle namespace |
| `c` | TSH cluster picker |
| `r` | Refresh current tab |
| `y` | View YAML |
| `d` | Describe resource |
| `H` | Helm list |
| `T` | Cycle theme |
| `?` | Help overlay |
| `q` / `Ctrl+C` | Quit |

### Pods tab

| Key | Action |
|-----|--------|
| `l` | Tail logs (last 200 lines, live) |
| `s` | Tail a log file path inside the pod |
| `e` | Exec into container shell |
| `p` | Start port-forward |
| `P` | View / stop active port-forwards |
| `/` | Fuzzy search |

### Deployments tab

| Key | Action |
|-----|--------|
| `s` | Scale replicas |
| `r` | Rollout restart (with confirmation) |

### Secrets tab

| Key | Action |
|-----|--------|
| `Enter` | Decoded key/value view |
| `y` | Raw base64 YAML |
| `e` | Edit a secret key value |

## Prerequisites

- A valid kubeconfig (`kubectl` must work)
- For TSH cluster switching: [Teleport](https://goteleport.com/) `tsh` CLI

## Architecture

| Component | Crate |
|-----------|-------|
| TUI rendering | `ratatui` + `crossterm` |
| Kubernetes API | `kube-rs` (native client, no kubectl shelling) |
| Async runtime | `tokio` |
| Fuzzy search | `fuzzy-matcher` |

## License

MIT
