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

Die 13 DCPS-PyClasses werden bei Doc-Build per ``autodoc_mock_imports``
gemockt (das Extension-Module ``zerodds._core`` ist bei Sphinx-Doc-Build
nicht zwingend kompiliert). Vollstaendige Methoden-Signaturen entstehen
beim Doc-Build, wenn ``maturin develop --features extension-module``
vorab gelaufen ist; dann wird ``autodoc_mock_imports`` ueberschrieben.

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
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

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
