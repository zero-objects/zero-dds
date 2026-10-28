"""Tests for §6.7 — the sphinx documentation tree.

`sphinx-build` itself is not run here: it needs sphinx installed, and the
inventory is the part of §6.7 that can drift silently. What this file
checks is that the tree is complete and consistent with the module —
every class the extension module exports has an ``autoclass`` directive,
every pure-Python module an ``automodule`` one, every ``toctree`` entry a
file, and every ``literalinclude`` a script.
"""

from __future__ import annotations

import ast
import pathlib
import re

import pytest

import zerodds

_DOCS = pathlib.Path(__file__).resolve().parents[2] / "docs"
_EXAMPLES = pathlib.Path(__file__).resolve().parents[2] / "examples"
_PACKAGE = pathlib.Path(zerodds.__file__).resolve().parent

# PyClasses the vendor spec's §1.2 table names; the extension exports more,
# which `test_api_rst_documents_every_extension_class` covers.
_SPEC_PYCLASSES = (
    "DomainParticipantFactory",
    "DomainParticipant",
    "BytesTopic",
    "ShapeTopic",
    "Publisher",
    "Subscriber",
    "BytesWriter",
    "BytesReader",
    "Shape",
    "ShapeWriter",
    "ShapeReader",
    "GuardCondition",
    "WaitSet",
)

# The pure-Python modules the vendor spec's §1.1 names. The IDL path is
# generated code over `runtime/python`, reached through `topic`.
_SPEC_MODULES = ("loader", "topic", "aio")


def _api_rst() -> str:
    return (_DOCS / "api.rst").read_text(encoding="utf-8")


def _documented_classes() -> set[str]:
    return {
        m.group(1)
        for m in re.finditer(r"^\.\. autoclass:: zerodds\.(\w+)$", _api_rst(), re.M)
    }


def _documented_modules() -> set[str]:
    return {
        m.group(1)
        for m in re.finditer(r"^\.\. automodule:: zerodds\.(\w+)$", _api_rst(), re.M)
    }


def test_conf_py_is_importable_and_names_the_workspace_version() -> None:
    """The doc build reads the version rather than repeating it."""
    namespace: dict[str, object] = {"__file__": str(_DOCS / "conf.py")}
    exec(compile((_DOCS / "conf.py").read_text(encoding="utf-8"), "conf.py", "exec"), namespace)  # noqa: S102
    release = namespace["release"]
    assert isinstance(release, str) and release
    # Anchored on this file and not on the installed package. The wheel
    # lands in site-packages when CI installs it, and `parents[4]` from
    # there is `/usr/local` — the assertion then read `/usr/local/Cargo.toml`
    # and failed on a missing file rather than on a version that drifted.
    # `_DOCS` above uses the same anchor.
    manifest = pathlib.Path(__file__).resolve().parents[4] / "Cargo.toml"
    assert f'version = "{release}"' in manifest.read_text(encoding="utf-8")


def test_conf_py_declares_the_four_extensions_the_spec_names() -> None:
    """§6.7 — autodoc, napoleon, viewcode, intersphinx, plus the mock that
    lets the tree build without a compiled `_core`."""
    namespace: dict[str, object] = {"__file__": str(_DOCS / "conf.py")}
    exec(compile((_DOCS / "conf.py").read_text(encoding="utf-8"), "conf.py", "exec"), namespace)  # noqa: S102
    extensions = namespace["extensions"]
    assert isinstance(extensions, list)
    for wanted in (
        "sphinx.ext.autodoc",
        "sphinx.ext.napoleon",
        "sphinx.ext.viewcode",
        "sphinx.ext.intersphinx",
    ):
        assert wanted in extensions, f"conf.py is missing {wanted}"
    assert namespace["autodoc_mock_imports"] == ["zerodds._core"]


@pytest.mark.parametrize("name", _SPEC_PYCLASSES)
def test_api_rst_documents_every_spec_pyclass(name: str) -> None:
    """§6.7 — api.rst lists the PyClasses of the §1.2 table."""
    assert name in _documented_classes(), f"api.rst has no autoclass for {name}"


@pytest.mark.parametrize("name", _SPEC_MODULES)
def test_api_rst_documents_every_pure_python_module(name: str) -> None:
    """§6.7 — api.rst lists the three pure-Python modules of §1.1."""
    assert name in _documented_modules(), f"api.rst has no automodule for {name}"


@pytest.mark.skipif(
    not getattr(zerodds, "_CORE_AVAILABLE", False),
    reason="zerodds._core not compiled — maturin develop needed",
)
def test_api_rst_documents_every_extension_class() -> None:
    """Nothing the extension module exports is left out of the reference. A
    PyClass added to `_core` without a directive would otherwise be invisible.

    Classes defined in a pure-Python module are not listed here: their
    `automodule` covers them, which is what the previous test checks.
    """
    exported = set()
    for name in zerodds.__all__:
        member = getattr(zerodds, name, None)
        if isinstance(member, type) and getattr(member, "__module__", "") == "zerodds_py":
            exported.add(name)
    assert exported, "no `_core` PyClass reached the outer namespace"
    missing = sorted(exported - _documented_classes())
    assert not missing, f"exported but undocumented: {missing}"


def test_api_rst_documents_every_pure_python_module_in_the_package() -> None:
    """Same for the modules: every `.py` beside `__init__` is in api.rst."""
    present = {
        path.stem
        for path in _PACKAGE.glob("*.py")
        if path.stem != "__init__"
    }
    missing = sorted(present - _documented_modules())
    assert not missing, f"module present but undocumented: {missing}"


def test_toctree_entries_exist() -> None:
    index = (_DOCS / "index.rst").read_text(encoding="utf-8")
    body = index.split(".. toctree::", 1)[1]
    entries = []
    for line in body.splitlines()[1:]:
        stripped = line.strip()
        if not stripped:
            if entries:
                break
            continue
        if stripped.startswith(":"):
            continue
        entries.append(stripped)
    assert entries == ["quickstart", "examples", "api"]
    for entry in entries:
        assert (_DOCS / f"{entry}.rst").is_file(), f"toctree names a missing {entry}.rst"


def test_literalincludes_point_at_existing_examples() -> None:
    """The three §1.1 examples and native descriptor consumer stay included."""
    text = (_DOCS / "examples.rst").read_text(encoding="utf-8")
    included = re.findall(r"^\.\. literalinclude:: (\S+)$", text, re.M)
    assert included == [
        "../examples/01_bytes_pubsub.py",
        "../examples/02_shape_pubsub.py",
        "../examples/03_idl_topic.py",
        "../tests/generated_descriptor_live.py",
    ], f"documented examples or native descriptor consumer changed: {included}"
    for rel in included:
        path = (_DOCS / rel).resolve()
        assert path.is_file(), f"examples.rst includes a missing {rel}"
        ast.parse(path.read_text(encoding="utf-8"), filename=str(path))


@pytest.mark.parametrize(
    "script",
    ["01_bytes_pubsub.py", "02_shape_pubsub.py", "03_idl_topic.py"],
)
def test_examples_parse(script: str) -> None:
    """§1.1 — the three examples are present and syntactically whole."""
    path = _EXAMPLES / script
    assert path.is_file(), f"{path} is missing"
    ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
