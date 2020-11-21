color-backtrace
===============

[![Crates.io][crates-badge]][crates-url]
[![docs.rs][docs-badge]][docs-url]
[![MIT licensed][mit-badge]][mit-url]
[![Apache 2.0 licensed][apache-badge]][apache-url]

[crates-badge]: https://img.shields.io/crates/v/color-backtrace.svg
[crates-url]: https://crates.io/crates/color-backtrace
[docs-badge]: https://docs.rs/color-backtrace/badge.svg
[docs-url]: https://docs.rs/color-backtrace/
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE-MIT
[apache-badge]: https://img.shields.io/badge/license-Apache%202.0-blue.svg
[apache-url]: LICENSE-APACHE

A Rust library that makes panics a little less painful by nicely colorizing them
and printing the relevant source snippets.

```toml
[dependencies]
color-backtrace = { version = "0.5" }
```

To enable it, simply place this code somewhere in your app initialization code:
```rust
color_backtrace::install();
```

If you want to customize some settings, you can instead do:
```rust
use color_backtrace::{default_output_stream, BacktracePrinter};
BacktracePrinter::new().message("Custom message!").install(default_output_stream());
```

### Features
- Colorize backtraces to be easier on the eyes
- Show source snippets if source files are found on disk
- Print frames of application code vs dependencies in different color
- Hide all the frames after the panic was already initiated
- Hide language runtime initialization frames

### Reducing transitive dependencies

In order to reduce transitive dependencies, you can disable the default
enabled `gimli-symbolize` feature by adding a `default-features = false`
clause to your `Cargo.toml` dependency entry, e.g.:

```toml
[dependencies]
color-backtrace = { version = "0.5", default-features = false }
```

This will reduce dependencies from ~50 → ~10. However, you'll pay for it with
[inaccurate source info](https://github.com/athre0z/color-backtrace/issues/2)
on macOS and Linux

### Usage in tests

Unfortunately, defining custom init functions run before tests are started is
currently [not supported in Rust](https://github.com/rust-lang/rfcs/issues/1664).
Since initializing color-backtrace in each and every test is tedious even when
wrapping it into a function, I recommended using the
[ctor](https://crates.io/crates/ctor) crate for this.

Somewhere, preferably in your crate's main module, put the following code:
```rust
#[cfg(test)]
mod tests {
    use ctor::ctor;

    #[ctor]
    fn init_color_backtrace() {
        color_backtrace::install();
    }
}
```

You can also do this outside of a `#[cfg(test)]` section, in which case the
panic handler is installed for both test and regular runs.

### Screenshot
![Screenshot](https://i.imgur.com/jLznHxp.png)
