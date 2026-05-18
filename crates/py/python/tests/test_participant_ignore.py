"""Tests fuer §2.2 — DomainParticipant ignore_* + contains_entity.

Decken die `ignore_participant`/`ignore_topic`/`ignore_publication`/
`ignore_subscription`/`contains_entity`/`get_discovered_*` Branches
ab, die in den Smoke-Tests nicht aufgerufen wurden.
"""

from __future__ import annotations

import pytest

import zerodds

pytestmark = pytest.mark.skipif(
    not getattr(zerodds, "_CORE_AVAILABLE", False),
    reason="zerodds._core nicht kompiliert — maturin develop noetig",
)


def _offline_participant(domain_id: int = 110):
    return zerodds.DomainParticipantFactory.instance().create_participant_offline(
        domain_id,
    )


def test_ignore_participant_unknown_handle_is_silent():
    """§2.2 — ignore_participant mit unbekanntem Handle akzeptiert die
    Operation (DDS 1.4 §2.2.2.2.2.27: kein Match-Error, einfach Eintrag
    in der ignore-Liste)."""
    p = _offline_participant(111)
    p.ignore_participant(0xDEAD_BEEF)
    # Nach dem Call darf contains_entity weiterhin sauber antworten.
    assert p.contains_entity(0xDEAD_BEEF) is False


def test_ignore_topic_unknown_handle_is_silent():
    p = _offline_participant(112)
    p.ignore_topic(0xCAFE_F00D)
    assert p.contains_entity(0xCAFE_F00D) is False


def test_ignore_publication_unknown_handle_is_silent():
    p = _offline_participant(113)
    p.ignore_publication(0x1234_5678)
    assert p.contains_entity(0x1234_5678) is False


def test_ignore_subscription_unknown_handle_is_silent():
    p = _offline_participant(114)
    p.ignore_subscription(0x8765_4321)
    assert p.contains_entity(0x8765_4321) is False


def test_contains_entity_false_for_unknown_handle():
    """§2.2 — contains_entity gibt False fuer einen nie gesehenen Handle."""
    p = _offline_participant(115)
    assert p.contains_entity(0xAAAA_BBBB_CCCC_DDDD) is False


def test_get_discovered_topics_offline_is_empty():
    """§2.2 — get_discovered_topics auf offline-Participant ist leer."""
    p = _offline_participant(116)
    assert p.get_discovered_topics() == []


def test_get_discovered_participants_offline_is_empty():
    """§2.2 — get_discovered_participants auf offline-Participant ist leer."""
    p = _offline_participant(117)
    assert p.get_discovered_participants() == []


def test_discovered_participants_count_offline_is_zero():
    p = _offline_participant(118)
    assert p.discovered_participants_count() == 0


def test_assert_liveliness_offline_does_not_raise():
    """§2.2 — assert_liveliness ist eine wartungsfreie Operation und
    darf auf einem offline-Participant nicht raisen."""
    p = _offline_participant(119)
    p.assert_liveliness()  # darf nicht raisen
