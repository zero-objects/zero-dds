API Reference
=============

Pure-Python-Module
------------------

``zerodds.loader``
^^^^^^^^^^^^^^^^^^

.. automodule:: zerodds.loader
   :members:
   :undoc-members:
   :show-inheritance:

``zerodds.topic`` — ``IdlTopic`` / ``IdlWriter`` / ``IdlReader``
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. automodule:: zerodds.topic
   :members:
   :undoc-members:
   :show-inheritance:

``zerodds.aio``
^^^^^^^^^^^^^^^

.. automodule:: zerodds.aio
   :members:
   :undoc-members:
   :show-inheritance:

``zerodds.sample_state`` / ``zerodds.view_state`` / ``zerodds.instance_state``
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. automodule:: zerodds.sample_state
   :members:
   :undoc-members:
.. automodule:: zerodds.view_state
   :members:
   :undoc-members:
.. automodule:: zerodds.instance_state
   :members:
   :undoc-members:

PyO3-Extension-Module ``zerodds._core``
---------------------------------------

The native DCPS classes are mocked by this documentation configuration via
``autodoc_mock_imports = ["zerodds._core"]``. Building the extension with
``maturin develop`` does not automatically override that setting. This
documentation build therefore is not evidence that native signatures or
runtime behavior were exercised.

``DomainParticipantFactory``
^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. autoclass:: zerodds.DomainParticipantFactory
   :members:
   :undoc-members:
   :show-inheritance:

``DomainParticipant``
^^^^^^^^^^^^^^^^^^^^^

.. autoclass:: zerodds.DomainParticipant
   :members:
   :undoc-members:
   :show-inheritance:

``Publisher``
^^^^^^^^^^^^^

.. autoclass:: zerodds.Publisher
   :members:
   :undoc-members:
   :show-inheritance:

``Subscriber``
^^^^^^^^^^^^^^

.. autoclass:: zerodds.Subscriber
   :members:
   :undoc-members:
   :show-inheritance:

``BytesTopic`` / ``BytesWriter`` / ``BytesReader``
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. autoclass:: zerodds.BytesTopic
   :members:
   :undoc-members:
.. autoclass:: zerodds.BytesWriter
   :members:
   :undoc-members:
.. autoclass:: zerodds.BytesReader
   :members:
   :undoc-members:

``ShapeTopic`` / ``ShapeWriter`` / ``ShapeReader`` / ``Shape``
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. autoclass:: zerodds.ShapeTopic
   :members:
   :undoc-members:
.. autoclass:: zerodds.ShapeWriter
   :members:
   :undoc-members:
.. autoclass:: zerodds.ShapeReader
   :members:
   :undoc-members:
.. autoclass:: zerodds.Shape
   :members:
   :undoc-members:

``GuardCondition`` / ``WaitSet``
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. autoclass:: zerodds.GuardCondition
   :members:
   :undoc-members:
.. autoclass:: zerodds.WaitSet
   :members:
   :undoc-members:

``SampleInfo`` / ``BytesContentFilteredTopic``
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. autoclass:: zerodds.SampleInfo
   :members:
   :undoc-members:
.. autoclass:: zerodds.BytesContentFilteredTopic
   :members:
   :undoc-members:

``KeyedTopic`` / ``KeyedWriter`` / ``KeyedReader`` / ``KeyedReading``
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. autoclass:: zerodds.KeyedTopic
   :members:
   :undoc-members:
.. autoclass:: zerodds.KeyedWriter
   :members:
   :undoc-members:
.. autoclass:: zerodds.KeyedReader
   :members:
   :undoc-members:
.. autoclass:: zerodds.KeyedReading
   :members:
   :undoc-members:

``DataWriterQos`` / ``DataReaderQos``
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. autoclass:: zerodds.DataWriterQos
   :members:
   :undoc-members:
.. autoclass:: zerodds.DataReaderQos
   :members:
   :undoc-members:

``DataWriterListener`` / ``DataReaderListener``
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. autoclass:: zerodds.DataWriterListener
   :members:
   :undoc-members:
.. autoclass:: zerodds.DataReaderListener
   :members:
   :undoc-members:

``ReadCondition`` / ``QueryCondition``
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. autoclass:: zerodds.ReadCondition
   :members:
   :undoc-members:
.. autoclass:: zerodds.QueryCondition
   :members:
   :undoc-members:
