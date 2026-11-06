"""The DDS domains of the tests that go on the network.

A test that creates a participant with discovery (`create_participant`,
`create_participant_fast`, a subprocess, a foreign program) takes the
fixture `leased_domain`: a domain of the host's share of the band, leased
through the protocol of crates/test-domains (the Python copy in
tests/interop/omg/test_domains.py, which
crates/test-domains/tests/py_lease_agrees.rs holds to the Rust constants), so
that no other test process of the host and no CI job on the LAN takes it at
the same time (`ZERODDS_TEST_DOMAIN_BAND`, `ZERODDS_TEST_DOMAIN_DIR`). An
offline participant sends nothing and keeps a fixed domain.
"""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "tests" / "interop" / "omg"))
import test_domains  # noqa: E402


@pytest.fixture
def leased_domain():
    domain = test_domains.lease_run(1)
    yield domain
    test_domains.release(domain)
