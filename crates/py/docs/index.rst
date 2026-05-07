ZeroDDS Python Binding
======================

``zerodds`` ist das Python-Binding für den Rust-nativen DDS-Stack
ZeroDDS. API-Shape folgt bewusst OMG DDS 1.4 §2.2.2, damit Nutzer von
``cyclonedds-python`` oder ``rti.connextdds`` sich ohne Einarbeitung
zurechtfinden.

.. toctree::
   :maxdepth: 2
   :caption: Inhalt:

   quickstart
   examples
   api

Feature-Stand v1.3
------------------

* ``DomainParticipantFactory`` / ``DomainParticipant``
* ``BytesTopic`` + ``BytesWriter`` / ``BytesReader`` (opaker Payload)
* ``ShapeTopic`` + ``ShapeWriter`` / ``ShapeReader`` + ``Shape``
  (Cross-Vendor-Interop gegen Cyclone/Fast-DDS ShapesDemo)
* ``@idl_struct``-Decorator (XCDR2-LE byte-genau kompatibel mit Rust)
* Sync-Primitives: ``wait_for_matched_*``, ``wait_for_data``
* GIL-Release während blockierender DDS-Calls

Was kommt als nächstes
----------------------

* **v1.4** — nested Structs, ``sequence<T>``, Arrays, Unions,
  Optional; QoS-Profile aus XML/YAML.
* **v2.0** — DDS-Security-Plugin-Integration (AES-GCM-256 + PKI),
  ROS2-rcl-Passthrough.

Index
=====

* :ref:`genindex`
* :ref:`modindex`
* :ref:`search`
