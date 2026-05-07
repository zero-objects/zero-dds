Beispiele
=========

Im Verzeichnis ``crates/py/examples/`` stehen drei selbst-laufende
Skripte. Jedes wird ueber ``python examples/NN_foo.py`` gestartet.

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

03 — Eigener IDL-Typ per ``@idl_struct``
-----------------------------------------

.. literalinclude:: ../examples/03_idl_struct_cdr.py
   :language: python
   :linenos:

ROS2-Interop
------------

Wenn ``ros2``-Python installiert ist, kann man eine zerodds-Publisher
gegen einen ROS2-``std_msgs/String``-Subscriber laufen lassen, solange
der IDL-Decorator den korrekten ``typename`` traegt::

   @idl_struct(typename="std_msgs::msg::String")
   @dataclass
   class StdMsgsString:
       data: str

Der volle ROS2-IDL-Subset (``geometry_msgs``, ``sensor_msgs``) kommt
ueber den IDL→Python-Dataclass-Generator wenn dieser aktiviert wird.
