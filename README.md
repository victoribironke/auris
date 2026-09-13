# Auris

A fast, keyboard-first launcher for Windows, built with Rust and Slint.

## Current scope

The first vertical slice contains:

- A native desktop UI built with Slint
- Application discovery from the per-user and machine-wide Start Menu
- Fuzzy application search
- Keyboard-first result activation
- A provider-oriented structure ready for calculator, commands, and file search

## Development

Install the Rust toolchain, then run:

```sh
cargo run
```

The initial app index reads Start Menu `.lnk` files. Windows-specific global hotkey and window activation behavior will be added next, followed by usage ranking and background file indexing.
