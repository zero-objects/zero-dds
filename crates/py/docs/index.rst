ZeroDDS Python Binding
======================

``zerodds`` is the Python binding for the Rust-native DDS stack
ZeroDDS. The API shape deliberately follows OMG DDS 1.4 §2.2.2, so users
of ``cyclonedds-python`` or ``rti.connextdds`` find their way around
without ramp-up.

.. toctree::
   :maxdepth: 2
   :caption: Contents:

   quickstart
   examples
   api

Surface
-------

* ``DomainParticipantFactory`` / ``DomainParticipant``
* ``BytesTopic`` + ``BytesWriter`` / ``BytesReader`` (opaque payload)
* ``ShapeTopic`` + ``ShapeWriter`` / ``ShapeReader`` + ``Shape``
  (cross-vendor interop against Cyclone/Fast-DDS ShapesDemo)
* ``KeyedTopic`` + ``KeyedWriter`` / ``KeyedReader`` and ``SampleInfo``
  (keyed lifecycle: dispose, unregister, instance state)
* ``BytesContentFilteredTopic`` — a Python predicate over the payload
* ``IdlTopic`` with a ``zerodds-idlc generate --lang python`` descriptor
  over native typed entities: primitives, ``Sequence``/``Array``/
  ``Optional``/``Map``, enumerations, unions and nested aggregates. One
  CDR codec (``runtime/python``'s ``zerodds_wire``), same as C#/Java's
  generated ``TypeSupport`` companion — no separate hand-written one.
* ``DataWriterQos`` / ``DataReaderQos`` — all twenty-two policies
* ``DataWriterListener`` / ``DataReaderListener`` callbacks
* ``GuardCondition``, ``ReadCondition``, ``QueryCondition``, ``WaitSet``
* ``zerodds.aio`` — the same calls as coroutines
* ``zerodds.loader`` — the ``ctypes`` path over ``libzerodds``, no Rust
  build needed
* Sync primitives: ``wait_for_matched_*``, ``wait_for_data``
* GIL release during blocking DDS calls

Generated types and native identity
-----------------------------------

The shared ``zerodds-idlc generate --lang python`` path emits classes,
wire codecs and ``ZD_TOPIC_DESCRIPTORS``. The matching
``zerodds_wire.topic_descriptor`` runtime provides their codec and metadata
contract without creating DDS entities.

``IdlTopic(participant, name, generated.ZD_TOPIC_DESCRIPTORS[type_name])``
connects that contract to the native DDS entities. Complete metadata and
supported inverse key contexts are required. The Rust shared AST codec
handles receive-side key extraction and lifecycle reconstruction without
Python callbacks under DDS locks. See :doc:`examples` for the two-process
consumer and its boundaries.

Same-binding delivery is not proof of cross-vendor or ROS 2 support.

Index
=====

* :ref:`genindex`
* :ref:`modindex`
* :ref:`search`
