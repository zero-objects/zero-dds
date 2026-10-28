Quickstart
==========

Installation (dev setup)
------------------------

.. code-block:: bash

   python3 -m venv .venv
   source .venv/bin/activate
   pip install maturin pytest

   cd crates/py
   maturin develop --features extension-module

A minimal example
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

Dependency-free codec test
-----------------------------

The pure-Python modules (``zerodds.loader``, ``zerodds.topic``,
``zerodds.aio``) run **without a maturin build**. So does every generated
module ``zerodds-idlc generate --lang python`` writes, over
``runtime/python``'s ``zerodds_wire`` — the one Python CDR codec, needed
on ``PYTHONPATH`` alongside ``crates/py/python`` for the generated-IDL
tests. This lets unit tests in CI validate the Python layer without
having a Rust compiler ready. Native-only tests in that directory can be
skipped when the extension is absent. A successful pure-Python run is not
a native DDS delivery proof.

.. code-block:: bash

   cd crates/py
   PYTHONPATH="python:../../runtime/python" pytest python/tests/
