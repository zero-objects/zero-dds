Quickstart
==========

Installation (Dev-Setup)
------------------------

.. code-block:: bash

   python3 -m venv .venv
   source .venv/bin/activate
   pip install maturin pytest

   cd crates/py
   maturin develop --features extension-module

Ein Minimal-Beispiel
--------------------

.. code-block:: python

   import zerodds

   factory = zerodds.DomainParticipantFactory.instance()
   participant = factory.create_participant(0)

   topic = participant.create_bytes_topic("Chatter")
   publisher = participant.create_publisher()
   writer = publisher.create_bytes_writer(topic)

   subscriber = participant.create_subscriber()
   reader = subscriber.create_bytes_reader(topic)

   writer.wait_for_matched_subscription(1, timeout_secs=5.0)
   reader.wait_for_matched_publication(1, timeout_secs=5.0)

   writer.write(b"hello world")
   reader.wait_for_data(timeout_secs=3.0)
   for payload in reader.take():
       print(payload)

Dependency-freier Codec-Test
-----------------------------

Die reinen Python-Module ``zerodds.cdr`` und ``zerodds.idl`` laufen
**ohne maturin-Build**. Damit koennen Unit-Tests in CI den Python-
Layer validieren, ohne Rust-Compiler bereit zu halten.

.. code-block:: bash

   cd crates/py
   PYTHONPATH=python pytest python/tests/
