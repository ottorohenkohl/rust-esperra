# esperra

Sample `no_std` embedded Rust firmware for the **ESP32-C6**, built with
[`esp-hal`](https://github.com/esp-rs/esp-hal) and [`embassy`](https://embassy.dev/).

Scaffolded with [`esp-generate`](https://github.com/esp-rs/esp-generate)
(`--chip esp32c6 -o unstable-hal -o embassy -o esp-backtrace -o log -o ci`).

## What it does

Blinks a "Hello world!" log message once a second over the C6's USB-Serial-JTAG output,
using an `embassy` async task loop instead of a bare `loop {}`. It's meant as a starting
point, not a real application — replace the body of `main` in `src/bin/main.rs` with
your own logic.

## Developing

This is meant to be developed inside the **Embedded Rust** Coder workspace template
(`coder/esp32-rust`), which already has the Rust toolchain, the `riscv32imac-unknown-none-elf`
target, and `espflash` installed.

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
esptool.py --chip esp32c6 write_flash 0x0 firmware.bin
```

## CI

`.gitlab-ci.yml` runs on every push:
- `build:firmware` – `cargo build --release`, then merges the ELF into a flat
  `firmware.bin` via `espflash save-image --merge` and publishes it as an artifact
