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

03 — Custom IDL type via ``@idl_struct``
-----------------------------------------

.. literalinclude:: ../examples/03_idl_struct_cdr.py
   :language: python
   :linenos:

ROS2-Interop
------------

If ``ros2`` Python is installed, you can run a zerodds publisher
against a ROS2 ``std_msgs/String`` subscriber, as long as
the IDL decorator carries the correct ``typename``::

   @idl_struct(typename="std_msgs::msg::String")
   @dataclass
   class StdMsgsString:
       data: str

The full ROS2 IDL subset (``geometry_msgs``, ``sensor_msgs``) comes
via the IDL→Python dataclass generator when that is enabled.
