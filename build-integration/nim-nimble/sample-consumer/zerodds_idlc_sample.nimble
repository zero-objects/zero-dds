# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Sample consumer for the `before build` nimble hook — Nimble's own
# NimScript hook mechanism (idiomatic to nimble, the same category as
# dub's `preGenerateCommands` and a Cargo build.rs), not a wrapper shell
# script invoked outside the package manager.

version       = "0.1.0"
author        = "ZeroDDS Contributors"
description   = "Sample consumer proving the zerodds-idlc nimble build-step integration"
license       = "Apache-2.0"
srcDir        = "src"
bin           = @["main"]

requires "nim >= 1.6.0"

# Runs before every `nimble build`/`nimble run`, regenerating
# src/gen/Robot.nim from idl/Robot.idl. `zerodds-idlc` itself only
# rewrites the file when invoked — this hook re-invokes it unconditionally
# (nimble's hook API has no built-in staleness/manifest primitive to key
# off, unlike Mix.Task.Compiler's `manifests/0` or Gradle's
# `@InputFiles`/`@OutputDirectory` — flagged below, not silently assumed
# incremental).
before build:
  mkDir("src/gen")
  exec "zerodds-idlc generate idl/Robot.idl --nim -o src/gen"
