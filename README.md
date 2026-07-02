# KDrover

Rust rewrite of [discord-drover](https://github.com/hdrover/discord-drover) — a `version.dll` proxy that forces Discord for Windows to use an HTTP or SOCKS5 proxy and applies UDP tweaks for voice chat restrictions.

## Build

Requires Rust and a Windows target:

```powershell
cargo build --release
```

Outputs:

- `target/release/version.dll` — inject into Discord's app folder
- `target/release/kdrover.exe` — Material 3 installer (GUI)
- `target/release/kdrover-cli.exe` — command-line installer

## GUI installer

```powershell
cargo build --release -p kdrover-gui
.\target\release\kdrover.exe
```

The window matches the original Drover installer:

- HTTP / SOCKS5 / Direct modes
- Host, port, authentication fields
- Install and Uninstall buttons
- GitHub link in the app bar

Place `version.dll` next to `kdrover.exe` before installing (or build the whole workspace).

## CLI install

```powershell
cargo build --release
.\target\release\kdrover-cli.exe install --proxy "http://127.0.0.1:8080"
```

Direct mode (UDP bypass only, no proxy):

```powershell
.\target\release\kdrover-cli.exe install
```

Manual install: copy `version.dll` and `drover.ini` next to `Discord.exe`.

## Configuration

`drover.ini`:

```ini
[drover]
proxy = http://127.0.0.1:8080
udp-bypass = auto
udp-keepalive = 15
```

`udp-keepalive` is the interval, in seconds, at which the UDP fake packet is re-sent on
active voice sockets so DPI keeps the flow misclassified for the whole call (set `0` to
send it only once at connection start). See below.

Supported formats:

- `http://host:port`
- `http://user:pass@host:port`
- `socks5://127.0.0.1:1080`

Optional `drover-packet.bin` in the same folder sends extra UDP payload before the built-in voice bypass.

## CLI

```powershell
kdrover-cli list
kdrover-cli install --proxy socks5://127.0.0.1:1080
kdrover-cli uninstall
```

## How it works

1. Discord loads `version.dll` from its own folder (DLL search order hijack).
2. The DLL forwards real `version.dll` exports from `%SystemRoot%\System32`.
3. Hooks on `GetEnvironmentVariableW`, `GetCommandLineW`, `CreateProcessW`, and Winsock APIs redirect TCP traffic through the configured proxy.
4. First UDP packets on voice connections are modified to bypass local restrictions.
5. On active voice sockets the fake UDP packet is re-sent every `udp-keepalive` seconds so
   stateful DPI does not re-throttle a long call (the ~5000 ms ping spikes that otherwise
   recur on unstable networks until Discord reconnects).

## License

MIT
