// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * Type-check only — never run. `tsc --noEmit` over this file (with a
 * fresh generation on `TS_PATH`, below) is the proof that a generated
 * `<Type>TypeSupport` companion satisfies `crates/ts-node/src/dds.ts`'s
 * own `TypeSupport<T>` — the interface `DomainParticipant.
 * createTypedTopic` actually takes — checked against the real
 * interface via a type-only import, not a hand-copied duplicate that
 * could drift out of step with it.
 *
 * A type-only import is erased entirely, so this file never needs
 * `koffi` (which `dds.ts`'s own runtime code imports unconditionally)
 * installed to be type-checked, and never creates a domain participant
 * — this is the structural half of "wire createTypedTopic to consume
 * it"; a live native topic is `crates/ts-node/examples/
 * topic-typed-smoke.ts`'s and `crates/ts-node/test/smoke.test.ts`'s.
 *
 * Usage: see README.md — generate `support.idl` fresh into this same
 * directory (this file's own `./support.js` and `../../../../crates/
 * ts-node/src/dds.js` imports are both relative to it), `tsc --noEmit`
 * over both, then remove the generated file again.
 */

import type { TypeSupport } from "../../../../crates/ts-node/src/dds.js";
import { bridge } from "./support.js";

const point: TypeSupport<bridge.Point> = bridge.PointTypeSupport;
const named: TypeSupport<bridge.Named> = bridge.NamedTypeSupport;
const shortName: TypeSupport<bridge.ShortName> = bridge.ShortNameTypeSupport;
const extended: TypeSupport<bridge.Extended> = bridge.ExtendedTypeSupport;
const empty: TypeSupport<bridge.Empty> = bridge.EmptyTypeSupport;

// Read, so an unused-import diagnostic under a stricter caller
// configuration cannot mask the assignments above being the real check.
void [point, named, shortName, extended, empty];
