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
a heating-element output, and a one-second temperature measurement interval. Hardware pins
currently default to GPIO4 for the OneWire bus and GPIO5 for the heating element.

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
- `build:firmware` – uses the pinned `espressif/idf-rust:esp32s3_1.88.0.0` image, which
  already ships the Xtensa Rust toolchain and `espflash` preinstalled. Sources
  `export-esp.sh`, then runs `cargo build --release` and merges the ELF into a flat
  `firmware.bin` via `espflash save-image --merge`, publishing it as an artifact.
