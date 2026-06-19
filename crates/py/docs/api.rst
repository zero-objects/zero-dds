API-Referenz
============

Pure-Python-Module
------------------

``zerodds.cdr``
^^^^^^^^^^^^^^^

.. automodule:: zerodds.cdr
   :members:
   :undoc-members:
   :show-inheritance:

``zerodds.idl``
^^^^^^^^^^^^^^^

.. automodule:: zerodds.idl
   :members:
   :undoc-members:
   :show-inheritance:

``zerodds.loader``
^^^^^^^^^^^^^^^^^^

.. automodule:: zerodds.loader
   :members:
   :undoc-members:
   :show-inheritance:

PyO3-Extension-Module ``zerodds._core``
---------------------------------------

The 13 DCPS PyClasses are mocked during the doc build via
``autodoc_mock_imports`` (the extension module ``zerodds._core`` is not
necessarily compiled during the Sphinx doc build). Full method
signatures are produced during the doc build when
``maturin develop --features extension-module`` has been run
beforehand; then ``autodoc_mock_imports`` is overridden.

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
