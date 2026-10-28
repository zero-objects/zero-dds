#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Run the rosidl generators over one interface package, without ament.

The supported way to reach `rosidl_generator_c` and
`rosidl_typesupport_fastrtps_c` is `rosidl_generate_interfaces()` in an
ament CMake package. That is a package build per corpus case: a CMake
configure, a colcon-shaped install tree and a shared library, for a few
hundred bytes of output. This calls the generators the way
`rosidl_cmake` does — they take one JSON arguments file and nothing
else — so a case costs three template expansions.

Nothing here decides a wire question. Every byte the probe reports comes
out of the generated code this writes, unread.

    rosidl-generate.py --package zdros --idl-dir gen --out gen \
                       --message Robot --message CommonCell

Writes `<out>/<package>/msg/...` and prints one `NAME=value` line per
name the probe template needs.
"""

import argparse
import json
import os
import sys

# The three generators the probe uses, as (python package, entry point,
# resource directory). `rosidl_generator_c` gives the structure and its
# `__init`/`__fini`; `rosidl_typesupport_fastrtps_c` gives the
# serialiser under test; `rosidl_typesupport_introspection_c` gives the
# runtime type description `rmw_cyclonedds_cpp` serialises from, so the
# second ROS encoder is reachable from the same tree.
GENERATORS = [
    ("rosidl_generator_c", "generate_c", "rosidl_generator_c"),
    (
        "rosidl_typesupport_fastrtps_c",
        "generate_typesupport_fastrtps_c",
        "rosidl_typesupport_fastrtps_c",
    ),
    (
        "rosidl_typesupport_introspection_c",
        "generate_c",
        "rosidl_typesupport_introspection_c",
    ),
]


def visibility_control(prefix, resource, package, out_dir):
    """Write the header CMake would have substituted.

    Each generator ships `<name>__visibility_control.h.in` with two
    CMake placeholders and no `.em` template, because ament substitutes
    it with `configure_file`. The generated sources `#include` it
    unconditionally, so without it nothing compiles.
    """
    template = os.path.join(prefix, "share", resource, "resource",
                            resource + "__visibility_control.h.in")
    if not os.path.exists(template):
        return
    with open(template, encoding="utf-8") as handle:
        text = handle.read()
    text = text.replace("@PROJECT_NAME_UPPER@", package.upper())
    text = text.replace("@PROJECT_NAME@", package)
    target = os.path.join(out_dir, package, "msg",
                          resource + "__visibility_control.h")
    with open(target, "w", encoding="utf-8") as handle:
        handle.write(text)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", required=True)
    parser.add_argument("--idl-dir", required=True,
                        help="holds <package>/msg/<Name>.idl")
    parser.add_argument("--out", required=True)
    parser.add_argument("--message", action="append", default=[],
                        help="one per file, dependencies first")
    parser.add_argument("--prefix", default=os.environ.get("ROS_PREFIX", ""),
                        help="the ROS 2 installation prefix")
    args = parser.parse_args()

    if not args.prefix:
        parser.error("--prefix or $ROS_PREFIX is required")
    if not args.message:
        parser.error("--message is required at least once")

    # `rosidl_cmake` resolves an idl_tuple's relative half against the
    # package directory, and the generated `#include` lines are
    # `<package>/msg/...`. So the tuple's base is the directory that
    # *contains* `msg`, which is `<idl-dir>/<package>`.
    base = os.path.join(os.path.abspath(args.idl_dir), args.package)
    out_dir = os.path.abspath(args.out)
    package_out = os.path.join(out_dir, args.package)
    os.makedirs(os.path.join(package_out, "msg"), exist_ok=True)

    tuples = ["%s:msg/%s.idl" % (base, name) for name in args.message]
    arguments = {
        "package_name": args.package,
        "output_dir": package_out,
        "template_dir": "",
        "idl_tuples": tuples,
        # Empty: this is the timestamp set `generate_files` compares
        # against to decide whether an output is stale. With none, every
        # output is written every time, which is what a probe wants —
        # a stale header from another case would be a silent wrong
        # answer.
        "target_dependencies": [],
    }

    sys.path.insert(0, os.path.join(args.prefix, "lib",
                                    "python%d.%d" % sys.version_info[:2],
                                    "site-packages"))
    from rosidl_cmake import convert_camel_case_to_lower_case_underscore

    arguments_path = os.path.join(out_dir, "rosidl-arguments.json")
    for module_name, entry_point, resource in GENERATORS:
        arguments["template_dir"] = os.path.join(args.prefix, "share",
                                                 resource, "resource")
        with open(arguments_path, "w", encoding="utf-8") as handle:
            json.dump(arguments, handle)
        module = __import__(module_name, fromlist=[entry_point])
        getattr(module, entry_point)(arguments_path)
        visibility_control(args.prefix, resource, args.package, out_dir)

    # Shell-quoted: the caller sources this, and the source lists hold
    # space-separated paths that would otherwise be read as commands.
    #
    # The three lists are kept apart because they are compiled by
    # different front ends. `__functions.c` and the introspection
    # `__type_support.c` are C and *only* C: the introspection one
    # assigns a `void *` to a typed pointer, which C allows and C++ does
    # not, so a C++ compiler rejects it outright. Only the Fast RTPS
    # `__type_support_c.cpp` is C++.
    root = args.message[-1]
    snake = convert_camel_case_to_lower_case_underscore

    def paths(suffix):
        return " ".join(
            "%s/msg/detail/%s%s" % (args.package, snake(name), suffix)
            for name in args.message)

    for name, value in [
        ("ROS_MESSAGE", root),
        ("ROS_SNAKE", snake(root)),
        ("ROS_CTYPE", "%s__msg__%s" % (args.package, root)),
        ("ROS_C_SOURCES", paths("__functions.c")),
        ("ROS_MESSAGE_INCLUDES", "\n".join(
            '#include "%s/msg/detail/%s__struct.h"\n'
            '#include "%s/msg/detail/%s__functions.h"'
            % (args.package, snake(name), args.package, snake(name))
            for name in args.message)),
        ("ROS_FASTRTPS_SOURCES", paths("__type_support_c.cpp")),
        ("ROS_INTROSPECTION_SOURCES", paths("__type_support.c")),
    ]:
        print("%s='%s'" % (name, value.replace("'", "'\\''")))


if __name__ == "__main__":
    main()
