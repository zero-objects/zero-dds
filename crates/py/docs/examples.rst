Examples
========

The ``crates/py/examples/`` directory contains three self-running
scripts. Each is started via ``python examples/NN_foo.py``.

01 — Bytes-Pub/Sub
------------------

.. literalinclude:: ../examples/01_bytes_pubsub.py
   :language: python
   :linenos:

02 — ShapeType (Cross-Vendor-Interop)
--------------------------------------

.. literalinclude:: ../examples/02_shape_pubsub.py
   :language: python
   :linenos:

03 — Custom IDL type via generated code
-----------------------------------------

``examples/03_idl_topic.idl`` is the source; ``03_idl_topic_gen.py``
(committed next to it) is ``zerodds-idlc generate --lang python``'s
output. There is no hand-written Python CDR codec here — the generated
``TopicDescriptor``'s ``encode()``/``decode()`` are the same codec every
native entity uses, the C#/Java bindings' equivalent of their generated
``TypeSupport`` companion.

.. literalinclude:: ../examples/03_idl_topic.py
   :language: python
   :linenos:

Generated IDL and ROS 2 boundary
--------------------------------

Changing ``03_idl_topic.idl``'s ``typename`` to ``std_msgs::msg::String``
does not make the native ``BytesTopic`` announce that IDL type by itself —
a fresh ``zerodds-idlc generate`` run and a real ``IdlTopic`` (below) are
what carry a type's real identity to a native endpoint. Installing ROS 2
Python packages alone does not close that gap.

The shared-CLI generation above emits ``ZD_TOPIC_DESCRIPTORS`` alongside the
generated classes and codecs, which is what ``zerodds.IdlTopic`` (below)
takes for native DDS identity, keys and lifecycle. The fresh-output
consumer lives at ``tools/idlc/tests/python-support/consumer.py`` in the
repository; it checks the descriptor and codec boundary only, without a
native endpoint. The fixed ShapeType example above remains a separate
native binding route.

Native generated descriptor consumer
------------------------------------

For the installed-package path, run ``tests/installed_wheel_consumer.sh``
with a directory containing both matching wheels, the IDL compiler executable
and an unused DDS domain. It installs offline into a fresh virtual environment,
generates the existing ``NestedKey`` fixture and runs its native exchange.
Neither the binding nor the wire runtime is imported from the checkout.

``tests/generated_descriptor_live.py`` runs a reader in a second process and
checks the generated value, keyed handle and dispose reconstruction. Supply
the freshly generated ``support.py`` from
``tools/idlc/tests/python-support/support.idl``, the matching ``runtime/python``
and the built binding on ``PYTHONPATH`` for source-development checks only.
Run with an unused DDS domain and
``Point``, ``Extended`` or dependent ``NestedKey`` as its two arguments. Generate separately for
``xcdr1`` and ``xcdr2`` when checking both representations.

.. literalinclude:: ../tests/generated_descriptor_live.py
   :language: python

This typed path loads the generated complete TypeObject dependency closure.
Scalar/string/enum, alias and nested structure keys have native inverse support.
Missing dependencies and unsupported key contexts refuse registration. These same-binding measurements
do not claim ROS 2 or cross-vendor qualification.
