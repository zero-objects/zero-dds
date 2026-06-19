#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Full ZeroDDS ↔ RabbitMQ AMQP interop e2e suite (both protocols + cross-stack).
# Requires the base from setup_rabbitmq.sh (RabbitMQ 4.0 + pika + qpid-proton).
# Run from a ZeroDDS checkout on the Linux test host (codepit).
#
#   Path A (AMQP 1.0):   ZeroDDS-1.0 ⇄ RabbitMQ ⇄ pika-0.9.1
#   Path B (AMQP 0.9.1): ZeroDDS-0.9.1 self + ⇄ pika-0.9.1 + → proton-1.0
#   Cross-stack:         ZeroDDS-1.0 ⇄ RabbitMQ ⇄ ZeroDDS-0.9.1
set -euo pipefail
export AMQP_RABBITMQ=1

echo "==> base check"
python3 "$(dirname "$0")/validate_base.py"

echo "==> Path A — AMQP 1.0 interop"
cargo test -p zerodds-amqp-endpoint --test rabbitmq_amqp10_e2e -- --ignored --nocapture

echo "==> Path B — AMQP 0.9.1 interop"
cargo test -p zerodds-amqp-0-9-1 --test rabbitmq_amqp091_e2e -- --ignored --nocapture

echo "==> Cross-stack — ZeroDDS-1.0 ⇄ ZeroDDS-0.9.1"
cargo test -p zerodds-amqp-endpoint --test rabbitmq_cross_stack_e2e -- --ignored --nocapture

echo "==> AMQP interop suite: ALL GREEN"
