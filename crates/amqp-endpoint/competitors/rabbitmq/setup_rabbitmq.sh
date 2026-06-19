#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Provisions the RabbitMQ interop base for the AMQP cross-protocol e2e suite.
# RabbitMQ 4.0 speaks BOTH AMQP 0.9.1 and AMQP 1.0 natively on port 5672
# (`message_containers` feature flag) — so a single broker backs both the
# AMQP-1.0 (Path A) and the AMQP-0.9.1 (Path B) ZeroDDS interop tests.
#
# Idempotent. Run on the Linux test host (codepit, Debian 13 LXC).
#
#   Broker:     localhost:5672  (AMQP 0.9.1 + 1.0)
#   Management: localhost:15672 (HTTP API + UI)
#   User:       zerodds / zerodds  (administrator, full perms on vhost "/")
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive

echo "==> installing rabbitmq-server (4.0+) + reference clients"
apt-get install -y rabbitmq-server python3-pika python3-qpid-proton

echo "==> ensuring the service is up"
systemctl enable --now rabbitmq-server 2>/dev/null || service rabbitmq-server start || true

echo "==> management plugin"
rabbitmq-plugins enable rabbitmq_management || true

echo "==> interop user (guest is localhost-only; zerodds is the test identity)"
rabbitmqctl add_user zerodds zerodds 2>/dev/null || true
rabbitmqctl set_user_tags zerodds administrator
rabbitmqctl set_permissions -p / zerodds ".*" ".*" ".*"

echo "==> listeners"
rabbitmqctl status | grep -A4 -i "Listeners" || true

echo "==> done. validate with: validate_base.py"
