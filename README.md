color-backtrace
===============

A Rust library that makes panics a little less painful by nicely colorizing them
and printing the relevant source snippets.

```toml
[dependencies]

color-backtrace = { version = "*" }
```

To enable it, simply place this code somewhere in your app initialization code:
```rust
color_backtrace::install();
```