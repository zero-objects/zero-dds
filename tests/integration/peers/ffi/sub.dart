// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The Dart subscriber of tests/integration/tests/bindings_ffi.rs: the dart:ffi
// loader of zerodds-ffi-loader-1.0 §3.6 (the Flutter binding of the dds-chat
// tutorial, examples/tutorials/dds-chat/apps/flutter-mobile/lib/native/
// loader.dart) over libzerodds, with ZERODDS_LIB naming the library. Run by
// the Dart SDK of Flutter with the app's package configuration, which holds
// `package:ffi`:
//
//   dart --packages=<app>/.dart_tool/package_config.json sub.dart \
//       <domain> <topic> <type name> <samples>
//
// The protocol of sub.c: `EVENT matched`, `EVENT sample <hex>` per body,
// `EVENT received n=<count>`, one more take after standard input closed,
// `EVENT done`.
import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import '../../../../examples/tutorials/dds-chat/apps/flutter-mobile/lib/native/loader.dart';

String _hex(Uint8List bytes) =>
    bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();

bool _takeOne(Reader reader) {
  final body = reader.take();
  if (body == null) return false;
  stdout.writeln('EVENT sample ${_hex(body)}');
  return true;
}

Future<int> _run(List<String> argv) async {
  if (argv.length != 4) {
    stderr.writeln('usage: sub.dart <domain> <topic> <type name> <samples>');
    return 2;
  }
  final domain = int.parse(argv[0]);
  final wanted = int.parse(argv[3]);
  final runtime = Runtime(domain);
  final reader = Reader(runtime, argv[1], argv[2], reliable: true);
  try {
    try {
      reader.waitForMatched(minCount: 1, timeoutMs: 10000);
    } on ZeroDdsException catch (e) {
      stdout.writeln('EVENT unmatched $e');
      return 1;
    }
    stdout.writeln('EVENT matched');
    var taken = 0;
    final deadline = DateTime.now().add(const Duration(seconds: 10));
    while (taken < wanted && DateTime.now().isBefore(deadline)) {
      if (_takeOne(reader)) {
        taken += 1;
      } else {
        // The loader has no blocking take.
        await Future<void>.delayed(const Duration(milliseconds: 5));
      }
    }
    stdout.writeln('EVENT received n=$taken');
    await stdin.transform(utf8.decoder).drain<void>();
    while (_takeOne(reader)) {
      // A sample taken now was delivered twice.
    }
    stdout.writeln('EVENT done');
    return 0;
  } finally {
    reader.close();
    runtime.close();
  }
}

Future<void> main(List<String> argv) async {
  exitCode = await _run(argv);
  await stdout.flush();
}
