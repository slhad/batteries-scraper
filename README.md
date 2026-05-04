# batteries-scraper

Small Rust HTTP bridge for exposing Logitech mouse battery data on Linux in a format compatible with [`LGSTrayBattery`](https://github.com/andyvorld/LGSTrayBattery).

This was built to replace the Windows-side battery exporter for a Home Assistant `scrape` sensor.

## What It Does

- reads battery percentage from Linux `upower`
- serves a Windows-compatible XML payload on `http://192.168.0.118:12321/device/dev00000000`
- can install itself as a `systemd --user` service

Example response:

```xml
<?xml version="1.0" encoding="UTF-8"?><xml><device_id>dev00000000</device_id><device_name>G502 LIGHTSPEED Wireless Gaming Mouse</device_name><device_type>Mouse</device_type><battery_percent>85.00</battery_percent><battery_voltage>0.00</battery_voltage><mileage>0.00</mileage><charging>false</charging><last_update>1777936225</last_update></xml>
```

## Requirements

- Linux
- Rust toolchain
- `upower`
- a Logitech mouse already exposed through `upower`
- if accessed from another machine: firewall rule allowing `12321/tcp`

## Build

```bash
cargo build --release
```

Binary:

```bash
./target/release/batteries-scraper
```

## Endpoints

- `GET /device/dev00000000`
  Returns LGSTrayBattery-compatible XML.
- `HEAD /device/dev00000000`
  Returns headers only.
- `GET /`
  Returns plain battery percentage.
- `GET /health`
  Returns a small JSON health payload.

## Configuration

Environment variables:

- `BIND_ADDR`
  Default: `192.168.0.118:12321`
- `UPOWER_DEVICE`
  Default: `/org/freedesktop/UPower/devices/battery_hidpp_battery_0`

Example:

```bash
BIND_ADDR=0.0.0.0:12321 \
UPOWER_DEVICE=/org/freedesktop/UPower/devices/battery_hidpp_battery_0 \
./target/release/batteries-scraper
```

## Install As User Service

Install and enable the user service:

```bash
./target/release/batteries-scraper --install-systemd-user
```

Useful commands:

```bash
systemctl --user status batteries-scraper.service
systemctl --user restart batteries-scraper.service
journalctl --user -u batteries-scraper.service -f
```

Installed unit path:

```text
~/.config/systemd/user/batteries-scraper.service
```

## Home Assistant

This project is intended to back a Home Assistant `scrape` integration pointing at:

```text
http://192.168.0.118:12321/device/dev00000000
```

If Home Assistant runs on another host, make sure the Linux firewall allows TCP `12321`.

Example `ufw` command:

```bash
sudo ufw allow 12321/tcp
```

Example `scrape` sensor configuration:

```yaml
sensor:
  - platform: scrape
    resource: http://192.168.0.118:12321/device/dev00000000
    name: G502 Battery
    select: "battery_percent"
    value_template: "{{ value | float }}"
    unit_of_measurement: "%"
    device_class: battery
    state_class: measurement
```

If you are configuring the integration through the Home Assistant UI instead of YAML, use:

- Resource: `http://192.168.0.118:12321/device/dev00000000`
- Selector: `battery_percent`
- Attribute / value template: `{{ value | float }}`
- Unit: `%`

## Test

```bash
cargo test
cargo fmt --check
```

Current unit tests cover:

- parsing `upower` output
- XML payload generation
- service file generation
- response content types
- shell escaping for systemd `ExecStart`
