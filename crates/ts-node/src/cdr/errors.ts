// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// errors.ts — XcdrError for encoder/decoder cases.

/// Exception class for all Xcdr2 layers. Subclass of
/// `Error`, so `instanceof Error` still matches.
export class XcdrError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'XcdrError';
    }
}
