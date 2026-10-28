/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * vendor_crash.h — a vendor library that dies on a sample or a payload
 * gives an answer, not an infrastructure failure.
 *
 * The pairwise runner reads an exit of 125 or more as a broken
 * measurement and stops. A probe that let a vendor's crash through
 * exited 134 or 139 and stopped the whole matrix: Cyclone serialising
 * a union whose `short` discriminator selects `case -1` takes the
 * `default` string branch and faults in `dds_stream_write_stringLE`;
 * Fast DDS reading a final union as an appendable one throws out of the
 * generated `_d()` setter through a `noexcept` frame into
 * `std::terminate`. Both are that vendor's answer to that case.
 *
 * Around the one call into the vendor, zd_vendor_call() turns SIGSEGV,
 * SIGBUS, SIGFPE, SIGILL and SIGABRT into a line on stderr and exit
 * ZD_VENDOR_CRASH_EXIT, which the runner records as the vendor not
 * writing or not reading the case, with that line as the reason.
 * zd_vendor_return() puts the default actions back, so a crash in the
 * probe's own fill or dump code still reads as the failure it is. */
#ifndef ZD_VENDOR_CRASH_H
#define ZD_VENDOR_CRASH_H

#include <signal.h>
#include <string.h>
#include <unistd.h>

#define ZD_VENDOR_CRASH_EXIT 6

static const char *zd_crash_vendor = "";
static const char *zd_crash_doing = "";
static const int zd_crash_signals[] = { SIGSEGV, SIGBUS, SIGFPE, SIGILL, SIGABRT };

static void zd_crash_say(const char *text) {
    ssize_t written = write(2, text, strlen(text));
    (void)written;
}

static void zd_crash_handler(int sig) {
    const char *name = sig == SIGSEGV ? "SIGSEGV"
                     : sig == SIGBUS  ? "SIGBUS"
                     : sig == SIGFPE  ? "SIGFPE"
                     : sig == SIGILL  ? "SIGILL"
                                      : "SIGABRT";
    zd_crash_say(zd_crash_vendor);
    zd_crash_say(" crashed (");
    zd_crash_say(name);
    zd_crash_say(") ");
    zd_crash_say(zd_crash_doing);
    zd_crash_say("\n");
    _exit(ZD_VENDOR_CRASH_EXIT);
}

static void zd_crash_set(void (*action)(int)) {
    struct sigaction sa;
    memset(&sa, 0, sizeof(sa));
    sa.sa_handler = action;
    sigemptyset(&sa.sa_mask);
    for (size_t i = 0; i < sizeof(zd_crash_signals) / sizeof(zd_crash_signals[0]); i++) {
        sigaction(zd_crash_signals[i], &sa, NULL);
    }
}

/* `vendor` and `doing` must outlive the call: string literals. */
static void zd_vendor_call(const char *vendor, const char *doing) {
    zd_crash_vendor = vendor;
    zd_crash_doing = doing;
    zd_crash_set(zd_crash_handler);
}

static void zd_vendor_return(void) {
    zd_crash_set(SIG_DFL);
}

#endif
