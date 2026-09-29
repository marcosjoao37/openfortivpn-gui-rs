# openfortivpn-gui-rs

> **⚠ Developed with AI assistance.** This project was built end-to-end by an AI coding agent (architecture, code, tests, packaging) with human review. Disclosed up front, as required by good sense and the GPL.

Linux desktop GUI for [`openfortivpn`](https://github.com/adrienverge/openfortivpn). Fortinet SSL VPN with a tray icon, profiles, and **no terminal**.

- **License:** GPL-3.0-or-later (see [`LICENSE`](LICENSE))
- **Platform:** Linux only — hard dependency on the `openfortivpn` binary
- **Stack:** Rust + `eframe`/egui + `tray-icon` (libayatana-appindicator), everything built inside Docker

**Start with [`AGENTS.md`](AGENTS.md).** It is the living spec: architecture (MVC boundaries), behavior contracts, security rules, decision log. Human or AI, read it before changing anything — it exists so future edits stay consistent.

---

## What it does

| Feature | Behavior |
|---|---|
| Connect / disconnect | Runs `sudo openfortivpn` for you; group-kill teardown (sudo + pppd) |
| Profiles | Multiple VPN profiles (server `host:port`, user, trusted-cert), dropdown + `+`/Edit modals, Delete inside editor. Stored in `~/.config/openfortivpn-gui/profiles.json` (0600) |
| Passwords | **Never saved.** VPN password typed every connect (passed via 0600 temp `--config` file, deleted on exit); sudo password — when sudoers isn't passwordless — asked once, kept in RAM (`Zeroizing`), cleared on exit |
| Sudoers handling | Probes `sudo -n <bin> --version`. No NOPASSWD? → password prompt. Tip panel with copyable `sudo EDITOR=nano visudo` + the exact rule line |
| Missing binary | Red banner, all fields + Connect disabled, distro-specific install command (`pacman/apt/dnf/zypper/apk/emerge`) with Copy button |
| Certificate trust | Per-profile SHA-256 `--trusted-cert`; unknown-cert fingerprints are parsed from openfortivpn output and offered via "Trust this certificate" |
| Tray | Open, Connect⇄Disconnect (dynamic), Status (live ↓↑ MB + KB/s), Exit (confirm when connected). Color-coded icon: gray idle / yellow connecting / green connected / red error |
| Window | Close (X) → hides to tray; Minimize → taskbar; single instance (second launch focuses the first) |
| Stats | Session totals in MB + down/up rates in KB/s, sampled 1 Hz from `/sys/class/net/ppp*/tun*` |
| External sessions | An openfortivpn started in a terminal is adopted at startup (shown connected, can be disconnected) |
| Notifications | Desktop notifications on connect, disconnect, and failures |
| About | Version, stack, AI-assistance disclosure |

## Install (user-level, no root)

```sh
make build          # docker: builds dist/openfortivpn-gui
make install-user   # → ~/.local/bin, ~/.local/share/applications, hicolor icons
```

Launch from your app menu ("openfortivpn GUI") or `~/.local/bin/openfortivpn-gui`. Uninstall: `make uninstall`.

Runtime deps (checked by the installer): `gtk3 libayatana-appindicator libxkbcommon-x11` — on Arch/CachyOS: `sudo pacman -S gtk3 libayatana-appindicator libxkbcommon-x11`.

## Develop

All compilation happens in Docker; the host needs only `docker` + `make`:

```sh
make dev-image      # archlinux:base-devel + rust + gtk3 + appindicator
make dev-run        # GUI with X11 passthrough
make fmt clippy test build icons
```

Layout is strict MVC — see [`AGENTS.md`](AGENTS.md) for the boundary rules (`view` never spawns processes, `controller` never imports egui), the channel/event model, and the verification checklist every change must pass (`cargo fmt`, `clippy -D warnings`, `cargo test`, smoke run).

## Security model (short version)

- VPN + sudo passwords: never on argv, never on disk. Secrets move through `Zeroizing<String>` channels
- Sudo: NOPASSWD probed; otherwise one in-app prompt per session, wiped on exit or auth failure
- Process teardown: `setsid` + process-group `SIGTERM`→`SIGKILL`, so no orphaned `pppd`
- First-run default profile is seeded from the maintainer's reference command; edit it in the UI, it's just JSON

## Repo

- `main` — stable
- `development` — work happens here
- remote: `git@github.com:marcosjoao37/openfortivpn-gui-rs.git`
- maintainer: João Marcos Silva e Araújo — [github.com/marcosjoao37](https://github.com/marcosjoao37)

## License

GPL-3.0-or-later. Full text in [`LICENSE`](LICENSE). AI-assisted development does not change your obligations under the license.
