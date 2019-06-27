color-backtrace
===============

[![Crates.io][crates-badge]][crates-url]
[![docs.rs][docs-badge]][docs-url]
[![MIT licensed][mit-badge]][mit-url]

[crates-badge]: https://img.shields.io/crates/v/color-backtrace.svg
[crates-url]: https://crates.io/crates/color-backtrace
[docs-badge]: https://docs.rs/color-backtrace/badge.svg
[docs-url]: https://docs.rs/color-backtrace/
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE

A Rust library that makes panics a little less painful by nicely colorizing them
and printing the relevant source snippets.

```toml
[dependencies]
color-backtrace = { version = "0.2" }
```

To enable it, simply place this code somewhere in your app initialization code:
```rust
color_backtrace::install();
```

If you want to customize some settings, you can instead do:
```rust
use color_backtrace::{install_with_settings, Settings};
install_with_settings(Settings::new().message("Custom message!"));
```

### Features
- Colorize backtraces to be easier on the eyes
- Show source snippets if source files are found on disk
- Print frames of application code vs dependencies in different color
- Hide all the frames after the panic was already initiated
- Hide language runtime initialization frames

### Optional Features

- **`failure-bt`** — Experimental support for printing `failure::Backtrace` backtraces.

### **Experimental** Failure backtrace integration

`failure` backtraces are opaque and so this feature uses unsafe code to
transmute the struct into a non private struct to allow access to the internal
`backtrace::Backtrace` object.

The code is dependent on and only tested against failure version `0.1.5` and is
considered a temporary hack while we work on getting backtraces from errors
exposed properly. User discretion is advised.

To enable, include the following in your Cargo.toml

```toml
[dependencies]
color-backtrace = { version = "0.2", features = ["failure-bt"] }
```

### Screenshot
![Screenshot](https://i.imgur.com/jLznHxp.png)
