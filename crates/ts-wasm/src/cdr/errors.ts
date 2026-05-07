// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// errors.ts — XcdrError fuer Encoder/Decoder-Faelle.

/// Exception-Klasse fuer alle Xcdr2-Schichten. Unterklasse von
/// `Error`, sodass `instanceof Error` weiterhin trifft.
export class XcdrError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'XcdrError';
    }
}
