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

Feature status v1.3
-------------------

* ``DomainParticipantFactory`` / ``DomainParticipant``
* ``BytesTopic`` + ``BytesWriter`` / ``BytesReader`` (opaque payload)
* ``ShapeTopic`` + ``ShapeWriter`` / ``ShapeReader`` + ``Shape``
  (cross-vendor interop against Cyclone/Fast-DDS ShapesDemo)
* ``@idl_struct`` decorator (XCDR2-LE byte-exact compatible with Rust)
* Sync primitives: ``wait_for_matched_*``, ``wait_for_data``
* GIL release during blocking DDS calls

What's next
-----------

* **v1.4** — nested Structs, ``sequence<T>``, Arrays, Unions,
  Optional; QoS-Profile aus XML/YAML.
* **v2.0** — DDS-Security-Plugin-Integration (AES-GCM-256 + PKI),
  ROS2-rcl-Passthrough.

Index
=====

* :ref:`genindex`
* :ref:`modindex`
* :ref:`search`
