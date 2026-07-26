# esperra

Sample `no_std` embedded Rust firmware for the **ESP32-S3**, built with
[`esp-hal`](https://github.com/esp-rs/esp-hal) and [`embassy`](https://embassy.dev/).

Scaffolded with [`esp-generate`](https://github.com/esp-rs/esp-generate)
(`--chip esp32s3 -o unstable-hal -o embassy -o esp-backtrace -o log -o ci`).

> Note: the ESP32-S3 is an **Xtensa** chip, not RISC-V. That means it needs Espressif's
> Rust fork toolchain (installed via [`espup`](https://github.com/esp-rs/espup)), unlike
> the ESP32-C6 the workspace template originally targeted. This project stays `no_std`
> (no ESP-IDF C toolchain needed) — only the compiler itself differs.

## What it does

Blinks a "Hello world!" log message once a second over the S3's USB-Serial-JTAG output,
using an `embassy` async task loop instead of a bare `loop {}`. It's meant as a starting
point, not a real application — replace the body of `main` in `src/bin/main.rs` with
your own logic.

## Developing

This is meant to be developed inside the **Embedded Rust** Coder workspace template
(`coder/esp32-rust`), which installs `espup`'s Xtensa Rust toolchain for `esp32s3`,
sets it as the default `rustup` toolchain, and has `espflash` ready to go.

```bash
cargo build --release
```

### Flashing from inside the workspace

Only works if the ESP32 is reachable by the workspace (uncommon for a remote pod):

```bash
cargo run --release   # uses the runner configured in .cargo/config.toml (espflash flash --monitor)
```

### Flashing from your local machine (recommended)

Every push to `main` builds the firmware in CI and produces a single merged, flashable
`firmware.bin` as a pipeline artifact — download it from the `build:firmware` job. On
your machine, install just `espflash` (`brew install espflash` / `cargo install espflash`)
or `esptool` (`pip install esptool`) — no Rust toolchain needed — and flash:

```bash
espflash flash --format raw firmware.bin /dev/tty.usbserial-XXXX
# or
esptool.py --chip esp32s3 write_flash 0x0 firmware.bin
```

## CI

`.gitlab-ci.yml` runs on every push:
- `build:firmware` – uses Espressif's `espressif/idf-rust:esp32s3_latest` image, which
  already ships the Xtensa Rust toolchain and `espflash` preinstalled. Sources
  `export-esp.sh`, then runs `cargo build --release` and merges the ELF into a flat
  `firmware.bin` via `espflash save-image --merge`, publishing it as an artifact.
