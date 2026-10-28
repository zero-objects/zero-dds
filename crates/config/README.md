# zerodds-config

Layered configuration trees with a base file, filename-sorted YAML drop-ins,
includes and per-value source locations. This crate loads and merges data.
It does not by itself define the DDS runtime's defaults or activation policy.

Maps merge recursively. Scalars and lists replace earlier values, while
`+name` appends to the list at `name`. `Config::origin_of` identifies the
surviving value's source. The parser accepts a deliberately limited YAML
subset, not arbitrary YAML. Include cycles and excessive nesting are errors.

`include:` accepts a file or directory path, or a block sequence of paths.
Included directories use the selected extension filter and filename order.
Their fragments may explicitly include further directories. Includes are
loaded before the including file's own keys, regardless of the directive's
textual position. Paths are relative to the including file and must exist.
Missing optional top-level drop-in directories remain valid. A refused
`Config::push` leaves the effective tree and source list unchanged.

Use `Loader` to select sources, then inspect the resulting `Config`. The
[library documentation](src/lib.rs) defines the syntax, precedence and
supported include forms. [Loader tests](tests) exercise these contracts.

Around the files sit two more layers, both in `layers.rs`: an environment
layer beneath them (`env_config`, from a consumer's table of variable names
and paths) and the `--set PATH=VALUE` flags above them (`CommandLine`).
`CommandLine::extract` takes `--config`, `--set` and `--check-config` out of
a program's arguments; `loader_for` selects the `--config` base or the
platform's `SystemLayout`; `Loader::load_onto` puts the files over the
environment; `unknown_keys` lists paths no consumer reads. The precedence is
defaults < environment < files < flags.

Environment interpolation (`${VAR}`) inside a file is separate from the
environment layer: it substitutes text while the file is read.
