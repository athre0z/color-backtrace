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
color-backtrace = { version = "0.7" }
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

### Screenshot

![Screenshot](https://i.imgur.com/yzp0KH6.png)

### Reducing transitive dependencies

It is possible to use the alternative [`btparse`] backtrace capturing backend
instead of the default route that links [`backtrace`]:

```toml
[dependencies]
color-backtrace = {
  version = "0.7",
  default-features = false,
  features = ["use-btparse-crate"],
}
```

This reduces the number of transitive dependencies from around 12 to just 2. So
why isn't this the default, you may ask? There's a stability tradeoff here:
`btparse` relies on the **undocumented and unstable** `std::fmt::Debug`
implementation of `std::backtrace::Backtrace` to remain unchanged. As of writing,
this has been untouched for 4+ years, but there's no *guarantee* that it will
always work.

[`btparse`]: https://github.com/yaahc/btparse
[`backtrace`]: https://github.com/rust-lang/backtrace-rs

<details>
<summary>Dependency tree with `use-backtrace-crate` (default)</summary>

```
$ cargo tree
color-backtrace v0.6.1 (/Users/ath/Development/color-backtrace)
├── backtrace v0.3.73
│   ├── addr2line v0.22.0
│   │   └── gimli v0.29.0
│   ├── cfg-if v1.0.0
│   ├── libc v0.2.155
│   ├── miniz_oxide v0.7.4
│   │   └── adler v1.0.2
│   ├── object v0.36.1
│   │   └── memchr v2.7.4
│   └── rustc-demangle v0.1.24
│   [build-dependencies]
│   └── cc v1.1.1
└── termcolor v1.4.1
```

</details>

<details>
<summary>Dependency tree with `use-btparse-crate`</summary>

```
$ cargo tree --no-default-features --features=use-btparse-crate
color-backtrace v0.6.1 (/Users/ath/Development/color-backtrace)
├── btparse v0.2.0 (https://github.com/yaahc/btparse.git?rev=54f9ddb8c7c8f8e034226fdcacab93cd76e1453b#54f9ddb8)
└── termcolor v1.4.1
```

</details>

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

### Environment Variable Support

The lib supports environment variables for controlling color output:

- **`NO_COLOR`**: When set (to any value), disables all colors in backtrace output
- **`FORCE_COLOR`**: When set (to any value), forces colors even when output is redirected
- **`NO_COLOR` takes precedence**: If both are set, `NO_COLOR` wins and colors are disabled

If none of the environement variables are provided, colors are automatically
applied if `stderr` is attached to a tty.

This follows the [`NO_COLOR` specification](https://no-color.org/).
