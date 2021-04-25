# Changelog
All notable changes to this project will be documented in this file.

## [v0.5.1] (2021-04-25)
- Add the ability to print module_name:offset, or address of frame
  - Contributed by [@s1341], thanks!

## [v0.5.0] (2020-11-21)

- Add `__rust_begin_short_backtrace` filter
- Remove experimental failure support

## [v0.4.2] (2020-05-19)

#### Added

- `Clone` and `Debug` impls for `BacktracePrinter`
- `COLORBT_SHOW_HIDDEN` env variable, disabling frame filtering

## [v0.4.1] (2020-05-08)

#### Fixed

- Use correct verbosity level for string formatting
- Fix off-by-one in frame hiding code
  - Hides one additional post-panic frame
- Slightly improved doc

## [v0.4.0] (2020-05-06)

#### Added
- `BacktracePrinter::format_trace_to_string`
- Ability to add custom frame filter callbacks
  - `BacktracePrinter::add_frame_filter`
  - `BacktracePrinter::clear_frame_filters`
  - `default_frame_filter`
  - Thanks to [@yaahc] for helping out with this!
- Prefer `RUST_LIB_BACKTRACE` env var when determining the default
  verbosity to print non-panic backtraces
  - Also contributed by [@yaahc]

#### Changed
- Rename `Settings` → `BacktracePrinter`
- Move `print_backtrace` → `BacktracePrinter::print_trace`
- Move `print_panic_info` → `BacktracePrinter::print_panic_info`
- Move `color_backtrace::failure::print_backtrace` →
  `BacktracePrinter::print_failure_trace`
- The majority of old APIs have deprecated shims that forward calls to
  their new place to ease porting
- The `out` setting is no longer part of the `BacktracePrinter` and instead
  supplied as an argument to all functions that need it
  - The previous design forced `Sync + Send + 'static` constraints
    on any output stream since they are required when registering
    the panic handler, but are unnecessary when printing to strings
  - As a bonus, all format and print functions no longer require
    mutable access to the `BacktracePrinter` instance

## [v0.3.0] (2019-11-12)

#### Added
- Custom `ColorScheme` support
- Forward backtrace-rs' `gimli-symbolize` feature, which is default enabled
  - This is done by adding `default-features = false` to the `Cargo.toml`
    dependency entry for `color-backtrace`
  - Disabling it reduces transitive dependencies from ~50 → ~10
  - However, you'll pay for it with [inaccurate source info](https://github.com/athre0z/color-backtrace/issues/2) on macOS
    and Linux

#### Changed
- Replace `term` crate for colorful term printing with `termcolor`
  - This crate is more actively maintained, has fewer deps and a better API
  - This made adding color scheme support very easy
- `Settings::dim_function_hash_part` was replaced
  - Hash part color is now controlled via `ColorScheme`

#### Removed
- `Colorize`, `ColorizedStderrOutput`, `StreamOutput`, `PanicOutputStream`
  - This functionality is now all provided by the `termcolor` crate
  - `termcolor` is re-exported in the root of `color_backtrace`
- Lots of transitive dependencies!

## [v0.2.3] (2019-08-23)

#### Changed
- Added post panic frame rules for [failure]
- Updated `term` dependency

## [v0.2.2] (2019-06-30)

#### Added
- Experimental support for [failure] error backtraces
    - Contributed by Jane Lusby ([@yaahc])

#### Changed
- Switch to [gimli] backend for backtraces on macOS and Linux
    - Fixes backtraces when invoking an app outside of its build directory
- Expose `print_backtrace` and `print_panic_info` functions

## [v0.2.1] (2019-06-25)

#### Changed
- Fixed panic then `TERM` env var is not found

## [v0.2.0] (2019-06-22)

#### Added
- This changelog!
- Customization via settings
- Printing to streams other than stderr

#### Changed
- Improved {dependency code,post panic frame,runtime init} heuristics
- Changed default panic message to be more professional
- Relicensed from MIT to MIT/Apache-2.0 dual licensing
- Moved `get_verbosity` → `Verbosity::from_env`
- Fix readability on light terminal themes
- Fix deadlock when unwrapping an error while printing the panic
- Many internal tweaks

[failure]: https://github.com/rust-lang-nursery/failure
[gimli]: https://github.com/gimli-rs/gimli
[@yaahc]: https://github.com/yaahc
[@s1341]: https://github.com/s1341

[v0.2.0]: https://github.com/athre0z/color-backtrace/releases/tag/v0.2.0
[v0.2.1]: https://github.com/athre0z/color-backtrace/releases/tag/v0.2.1
[v0.2.2]: https://github.com/athre0z/color-backtrace/releases/tag/v0.2.2
[v0.2.3]: https://github.com/athre0z/color-backtrace/releases/tag/v0.2.3
[v0.3.0]: https://github.com/athre0z/color-backtrace/releases/tag/v0.3.0
[v0.4.0]: https://github.com/athre0z/color-backtrace/releases/tag/v0.4.0
[v0.4.1]: https://github.com/athre0z/color-backtrace/releases/tag/v0.4.1
[v0.4.2]: https://github.com/athre0z/color-backtrace/releases/tag/v0.4.2
[v0.5.0]: https://github.com/athre0z/color-backtrace/releases/tag/v0.5.0

[bt-bug]: https://github.com/athre0z/color-backtrace/issues/2
