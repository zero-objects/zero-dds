#!/usr/bin/env bash
# preflight.sh — verifies the RT host config before we start a
# bench. Exit 0 if everything fits; exit 1 with a diagnosis otherwise.
#
# WP 5.D.5b Phase-C. Usage: `bash tests/perf/rt-tuning/preflight.sh`
# before each `cargo run -p dds-bench-suite --bin roundtrip-1us`.

set -euo pipefail

bad=0
warn=0

emit_fail() {
  echo "❌ FAIL: $*" >&2
  bad=$((bad + 1))
}
emit_warn() {
  echo "⚠ WARN: $*" >&2
  warn=$((warn + 1))
}
emit_ok() {
  echo "✓ OK:   $*"
}

# ---- 1) PREEMPT_RT ----
if [ -f /sys/kernel/realtime ] && [ "$(cat /sys/kernel/realtime)" = "1" ]; then
  emit_ok "PREEMPT_RT kernel active"
else
  emit_fail "no PREEMPT_RT kernel (/sys/kernel/realtime missing or != 1)"
fi

# ---- 2) Boot args: isolcpus / nohz_full / rcu_nocbs ----
cmdline=$(cat /proc/cmdline 2>/dev/null || echo "<unavailable>")
for need in isolcpus nohz_full rcu_nocbs; do
  if echo "$cmdline" | grep -q "$need="; then
    emit_ok "Boot arg $need= set: $(echo "$cmdline" | tr ' ' '\n' | grep "$need=")"
  else
    emit_fail "Boot arg $need= missing"
  fi
done

# ---- 3) CPU governor ----
gov=$(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null || echo "?")
if [ "$gov" = "performance" ]; then
  emit_ok "CPU governor=performance"
else
  emit_fail "CPU governor=$gov (expected: performance)"
fi

# ---- 4) Hyper-threading off ----
ht_active=$(lscpu 2>/dev/null | awk -F: '/^Thread\(s\) per core/ {gsub(/^[ \t]+|[ \t]+$/,"",$2); print $2}' || echo "?")
if [ "$ht_active" = "1" ]; then
  emit_ok "Hyper-threading off (1 thread per core)"
else
  emit_warn "Hyper-threading active (threads/core=$ht_active) — cache sharing can cause jitter"
fi

# ---- 5) C-States ----
if [ -d /sys/devices/system/cpu/cpu2/cpuidle ]; then
  for s in /sys/devices/system/cpu/cpu2/cpuidle/state*/disable; do
    name=$(cat "$(dirname "$s")/name")
    if [ "$name" != "POLL" ] && [ "$name" != "C1" ] && [ "$(cat "$s")" != "1" ]; then
      emit_warn "C-state $name on cpu2 not disabled"
    fi
  done
fi

# ---- 6) NIC tuning (eth0) ----
if command -v ethtool >/dev/null; then
  rx_usecs=$(ethtool -c eth0 2>/dev/null | awk '/^rx-usecs:/ {print $2}' || echo "?")
  if [ "$rx_usecs" = "0" ]; then
    emit_ok "eth0 rx-usecs=0 (no coalescing)"
  else
    emit_warn "eth0 rx-usecs=$rx_usecs (expected 0, otherwise interrupt bursts)"
  fi
fi

# ---- 7) cyclictest available ----
if command -v cyclictest >/dev/null; then
  emit_ok "cyclictest installed: $(cyclictest -V 2>&1 | head -1)"
else
  emit_warn "cyclictest missing — install with 'apt install rt-tests'"
fi

# ---- Summary ----
echo "---"
echo "checks: $bad fail, $warn warn"
if [ "$bad" -gt 0 ]; then
  echo "Host not RT-capable. See tests/perf/rt-tuning/host-config-checklist.md"
  exit 1
fi
echo "OK — host is ready for the bench."
exit 0
