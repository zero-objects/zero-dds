# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Unit tests for the interop result classifier (issue #28).

Pure-logic tests — no DDS stack required, runnable on any host including
macOS. Covers the five required scenarios (success, zero samples, decode
error, timeout, missing vendor binary) plus the result-line parser.
"""
import os
import sys

import pytest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from interop_result import (  # noqa: E402
    EXPECT_NEGATIVE,
    EXPECT_REFUSAL,
    EXPECT_SAMPLES,
    EXPECTED_NEGATIVE,
    EXPECTED_REFUSAL,
    PASS,
    PRODUCT_FAIL,
    SETUP_FAIL,
    TIMEOUT,
    Observed,
    classify,
    parse_result_line,
)


# ---- positive (samples) case ------------------------------------------------

def test_success_matched_samples_no_errors():
    obs = Observed(matched=1, samples=40, errors=0, discovered=1)
    assert classify(EXPECT_SAMPLES, obs) == PASS


def test_reverse_direction_no_match_count_but_samples():
    # Vendor reader reports only sample count (matched/errors unknown).
    obs = Observed(samples=39, matched=None, errors=None)
    assert classify(EXPECT_SAMPLES, obs) == PASS


def test_zero_samples_but_matched_is_product_fail():
    # Endpoint matched (discovery worked) but nothing decoded -> NOT a pass.
    # This is exactly the "SPDP alone is not interop" guard.
    obs = Observed(matched=1, samples=0, errors=0, discovered=1)
    assert classify(EXPECT_SAMPLES, obs) == PRODUCT_FAIL


def test_samples_with_decode_errors_is_product_fail():
    obs = Observed(matched=1, samples=10, errors=3)
    assert classify(EXPECT_SAMPLES, obs) == PRODUCT_FAIL


def test_timeout_never_matched_positive_case():
    obs = Observed(matched=0, samples=0, errors=0, discovered=0, timed_out=True)
    assert classify(EXPECT_SAMPLES, obs) == TIMEOUT


def test_matched_zero_samples_not_timed_out_is_product_fail():
    # Ran to completion, matched, zero samples, but the window simply ended:
    # product failure, not timeout, because a match was seen.
    obs = Observed(matched=1, samples=0, errors=0, timed_out=True)
    assert classify(EXPECT_SAMPLES, obs) == PRODUCT_FAIL


# ---- negative (expected decode error) case ----------------------------------

def test_expected_negative_visible_decode_error():
    # #27/#29 case: Cyclone @final XCDR2 vs ZeroDDS @appendable — matched,
    # zero decoded, decode errors surfaced via take() -> WireError.
    obs = Observed(matched=1, samples=0, errors=40, discovered=1)
    assert classify(EXPECT_NEGATIVE, obs) == EXPECTED_NEGATIVE


def test_negative_case_silent_timeout_is_timeout_not_pass():
    # A negative case that NEVER matched and just timed out must NOT be
    # rewarded as an expected-negative pass.
    obs = Observed(matched=0, samples=0, errors=0, timed_out=True)
    assert classify(EXPECT_NEGATIVE, obs) == TIMEOUT


def test_negative_case_but_samples_flowed_is_product_fail():
    # If the "incompatible" case actually delivered data, the negative
    # expectation is wrong -> product failure (regression signal).
    obs = Observed(matched=1, samples=20, errors=0)
    assert classify(EXPECT_NEGATIVE, obs) == PRODUCT_FAIL


# ---- setup failure ----------------------------------------------------------

def test_missing_vendor_binary_is_setup_fail():
    obs = Observed(setup_ok=False)
    assert classify(EXPECT_SAMPLES, obs) == SETUP_FAIL


def test_setup_fail_dominates_even_with_counts():
    obs = Observed(matched=1, samples=40, errors=0, setup_ok=False)
    assert classify(EXPECT_SAMPLES, obs) == SETUP_FAIL


def test_unknown_expectation_raises():
    with pytest.raises(ValueError):
        classify("bogus", Observed(samples=1))


# ---- refusal (expected non-delivery) case -----------------------------------

def test_expected_refusal_discovered_but_nothing_delivered():
    # #42 case: a Cyclone @final writer against a ZeroDDS @appendable reader,
    # both announcing their type. The pair is not assignable, so the correct
    # outcome is no connection and no octet.
    obs = Observed(matched=1, samples=0, errors=0, discovered=1)
    assert classify(EXPECT_REFUSAL, obs) == EXPECTED_REFUSAL


def test_refusal_with_delivered_samples_is_product_fail():
    # Samples crossed an incompatible pair: the peer did not check the type.
    obs = Observed(matched=1, samples=39, errors=0, discovered=1)
    assert classify(EXPECT_REFUSAL, obs) == PRODUCT_FAIL


def test_refusal_with_decode_errors_is_product_fail():
    # A decode error means a sample DID cross and broke on the wire — the
    # pre-#42 outcome, and not what this expectation asserts.
    obs = Observed(matched=1, samples=0, errors=39, discovered=1)
    assert classify(EXPECT_REFUSAL, obs) == PRODUCT_FAIL


def test_refusal_without_discovery_is_timeout_not_green():
    # Nothing was discovered inside the budget: the run proves nothing about
    # the type check and must not be rewarded as a refusal.
    obs = Observed(matched=0, samples=0, errors=0, discovered=0, timed_out=True)
    assert classify(EXPECT_REFUSAL, obs) == TIMEOUT


def test_refusal_without_discovery_and_no_timeout_is_product_fail():
    obs = Observed(matched=0, samples=0, errors=0, discovered=0)
    assert classify(EXPECT_REFUSAL, obs) == PRODUCT_FAIL


def test_refusal_setup_failure_dominates():
    obs = Observed(samples=0, errors=0, discovered=1, setup_ok=False)
    assert classify(EXPECT_REFUSAL, obs) == SETUP_FAIL


# ---- result-line parser -----------------------------------------------------

def test_parse_zerodds_reader_line():
    obs = parse_result_line("RESULT discovered=1 matched=1 samples=40 errors=0")
    assert (obs.discovered, obs.matched, obs.samples, obs.errors) == (1, 1, 40, 0)


def test_parse_vendor_reader_line_samples_only():
    obs = parse_result_line("CYCLONE_RESULT samples=39")
    assert obs.samples == 39
    assert obs.matched is None
    assert obs.errors is None


def test_parse_negative_line():
    obs = parse_result_line("RESULT discovered=1 matched=1 samples=0 errors=40")
    assert (obs.matched, obs.samples, obs.errors) == (1, 0, 40)


def test_parse_missing_fields_are_none():
    obs = parse_result_line("nothing useful here")
    assert obs.samples is None and obs.matched is None
