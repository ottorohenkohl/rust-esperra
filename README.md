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

Provides an ESP32-S3 boiler-control firmware built around a small typed actor facade over
Embassy. The composition root in `src/main.rs` wires a boiler, a DS18B20 temperature sensor,
a heating-element output, and a one-second temperature measurement interval. The heating output
uses GPIO21 so native USB Serial/JTAG on GPIO19/GPIO20 remains available for diagnostics.

## Project conventions

### Actors and messaging

Actors own their state and process exactly one mailbox message at a time through the asynchronous
`Actor::handle` method. A handler may await a timer or hardware operation; its mailbox does not
deliver another message until that handler finishes, while other actors continue running. Use the
small facade in `src/library/actor.rs` rather than Embassy channels directly. `ActorRef::tell` is
synchronous and fire-and-forget: it attempts to enqueue a message but never waits for the recipient
to process it.

Every actor defines a dedicated protocol whose variants describe the intended action of the
recipient. Prefer imperative names such as `MeasureTemperature`, `RegulateHeating`, `StopHeating`,
and `SetActive`. Neutral information published to multiple recipients uses a factual type such as
`TemperatureReading`. A recipient protocol includes such a published type through `From<T>`, and
`ActorRef::subscriber<T>` is available only when that conversion exists. Publishers store typed
`Subscriber<T>` values and must not know the protocols or identities of their recipients.

`Mailbox<Message, CAPACITY>` makes queue capacity explicit because capacity affects actor behavior.
It controls how many messages may wait, not how many are processed concurrently. `tell` uses
non-blocking delivery, and a message is currently dropped when its mailbox is full. Choose capacity
with periodic, redundant, and safety-relevant messages in mind.

### Domain and ports

Actors, protocols, published values, subscriber management, and decisions belong in `src/domain`.
External operations are represented by narrow domain traits whose names end in `Port`, for example
`ConfigPort`, `TemperaturePort`, `LightPort`, and `InputPort`. Adapter code in `src/adapter` owns the
actual ESP HAL, OneWire, GPIO, flash, and similar external APIs. Adapters implement ports; they do
not own actor protocols or actor behavior.

Actors use generic port injection consistently. For example, `TemperatureActor<P>` is generic over
`TemperaturePort`, and `BoilerActor<C>` and its internal `PidService<C>` are generic over the same
shared `ConfigPort` implementation. Prefer this static-dispatch pattern over `dyn Port`. Internal
services such as `PidService` are constructed by their owning actor so the composition root does not
need to know about internal dependencies.

Port operations return the shared `library::error::Error` instead of operation-specific hardware
error types. Hardware failures are treated uniformly unless the domain has a concrete reason to
distinguish them. Actors may translate a generic failure into a meaningful published value such as
`TemperatureReading::MeasurementFailed`.

Do not create wrappers that merely rename an external type without adding useful behavior. Direct
use of a focused external type, such as `embassy_time::Instant`, is acceptable when a custom
abstraction would only add indirection.

### Formatting

Formatting favors horizontal readability and minimal nesting. `rustfmt.toml` sets a very large line
width and maximum small-expression heuristics. Keep statements on one line where practical. Group
imports from the same direct module with braces, for example `use library::actor::{ActorRef,
Mailbox};`, but avoid nested import trees or braces inside braces. Prefer several flat imports when
modules differ.

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

Every push to `main` builds a rollback-enabled ESP-IDF bootloader, a merged `firmware.bin` for initial/recovery flashing, and an application-only `firmware-ota.bin`. Download `firmware.bin` from the `build:firmware` job for the first installation. On your machine, install just `espflash` (`brew install espflash` / `cargo install espflash`) or `esptool` (`pip install esptool`) - no Rust toolchain needed - and flash:

```bash
espflash flash --format raw firmware.bin /dev/tty.usbserial-XXXX
# or
esptool.py --chip esp32s3 write_flash 0x0 firmware.bin
```

## OTA updates

Turn the machine power off for 8–12 seconds, then back on, to check the configured firmware base URL and install a newer image into the inactive OTA partition. The image version is read from its ESP application descriptor and compared with the old version stored in the configuration. Before checking, the firmware stops brewing and sends a boiler safety shutdown; both indicator lights blink until the update completes. It synchronizes the RTC through SNTP and validates the download's HTTPS certificate and hostname against the embedded ISRG Root X1 certificate.

The ESP32-S3 module must provide at least 8 MB flash. Flash `firmware.bin` once over USB to install the custom bootloader and `res/partitions.csv`; subsequent releases use `firmware-ota.bin`. Existing devices also need this one-time USB flash before automatic rollback is available.

Configure these protected GitLab CI/CD variables before building:

- `ESPERRA_WIFI_SSID` and `ESPERRA_WIFI_PASSWORD` - Wi-Fi credentials compiled into the firmware.
- `ESPERRA_NTP_SERVER` - optional SNTP host; defaults to `pool.ntp.org`.
- `ESPERRA_FIRMWARE_URL` - default base URL used when the configuration has no firmware URL, for example `https://gitlab.rohenkohl.dev/api/v4/projects/38/packages/generic/firmware`.

A release requires the `Cargo.toml` version and tag to agree, for example version `0.2.0` with tag `v0.2.0`. The release job publishes a versioned Generic Package Registry package and replaces `latest/firmware-ota.bin`. Firmware downloads are public and require no GitLab credentials. Keep `res/isrg-root-x1.pem` current if the GitLab certificate chain changes to a different root CA.

The same 8–12 second power-off gesture confirms a newly booted OTA image. Until confirmation, the custom bootloader restores the previous valid OTA image after a reset or power loss. CI builds this bootloader from `boot/sdkconfig.defaults` with `CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=y` and supplies it to `espflash` when creating `firmware.bin`.

## CI

`.gitlab-ci.yml` builds the custom ESP-IDF bootloader, the merged recovery image, and the application-only OTA image on every push. A semantic-version tag additionally publishes immutable firmware and replaces the latest OTA image through GitLab's Generic Package Registry.
