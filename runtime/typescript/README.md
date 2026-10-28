# @zerodds/wire

The canonical TypeScript XCDR and descriptor runtime imported by generated
ZeroDDS modules. Consumers declare `@zerodds/wire` at the same version as
their compiler, currently `1.0.0-rc.7`. It is independent of the optional
`@zerodds/node` native DDS binding.

`npm pack` builds and packages JavaScript and declaration files from `src`
into `dist`. Install the resulting tarball in a separate consumer project
to verify package resolution. No repository path mapping is required.

The package manifest prepares publication but does not assert registry
availability. Package ownership and Trusted Publishing for `@zerodds/wire`
must be established before a separately authorized upload.
