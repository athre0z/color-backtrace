# Changelog
All notable changes to this project will be documented in this file.

## [v0.2.3] (2019-08-23)

### Changed
- Added post panic frame rules for [failure]
- Updated `term` dependency

## [v0.2.2] (2019-06-30)

### Added
- Experimental support for [failure] error backtraces
    - Contributed by Jane Lusby (@yaahallo)

### Changed
- Switch to [gimli] backend for backtraces on macOS and Linux
    - Fixes backtraces when invoking an app outside of its build directory
- Expose `print_backtrace` and `print_panic_info` functions

## [v0.2.1] (2019-06-25)

### Changed
- Fixed panic then `TERM` env var is not found

## [v0.2.0] (2019-06-22)

### Added
- This changelog!
- Customization via settings
- Printing to streams other than stderr

### Changed
- Improved {dependency code,post panic frame,runtime init} heuristics
- Changed default panic message to be more professional
- Relicensed from MIT to MIT/Apache-2.0 dual licensing
- Moved `get_verbosity` -> `Verbosity::from_env`
- Fix readability on light terminal themes
- Fix deadlock when unwrapping an error while printing the panic
- Many internal tweaks

[failure]: https://github.com/rust-lang-nursery/failure
[gimli]: https://github.com/gimli-rs/gimli

[v0.2.0]: https://github.com/athre0z/color-backtrace/releases/tag/v0.2.0
[v0.2.1]: https://github.com/athre0z/color-backtrace/releases/tag/v0.2.1
[v0.2.2]: https://github.com/athre0z/color-backtrace/releases/tag/v0.2.2