// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// DDS-AMQP 1.0 §7.1.3 — the two halves of a DDS `Time_t`, as a generated
// carrier declares them. Here and not in the generated file because every
// generated file that mentions a `Time_t` would otherwise declare its own.

package org.zerodds.amqp;

public final class TimeT {
    public long sec;
    public long nanosec;

    public TimeT() {}

    public TimeT(long sec, long nanosec) {
        this.sec = sec;
        this.nanosec = nanosec;
    }
}
