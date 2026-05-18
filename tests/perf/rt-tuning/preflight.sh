#!/usr/bin/env bash
# preflight.sh — Verifiziert RT-Host-Config bevor wir ein Bench
# starten. Exit 0 wenn alles passt; exit 1 mit Diagnose sonst.
#
# WP 5.D.5b Phase-C. Aufruf: `bash tests/perf/rt-tuning/preflight.sh`
# vor jedem `cargo run -p dds-bench-suite --bin roundtrip-1us`.

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
  emit_ok "PREEMPT_RT-Kernel aktiv"
else
  emit_fail "kein PREEMPT_RT-Kernel (/sys/kernel/realtime fehlt oder != 1)"
fi

# ---- 2) Boot-Args: isolcpus / nohz_full / rcu_nocbs ----
cmdline=$(cat /proc/cmdline 2>/dev/null || echo "<unavailable>")
for need in isolcpus nohz_full rcu_nocbs; do
  if echo "$cmdline" | grep -q "$need="; then
    emit_ok "Boot-Arg $need= gesetzt: $(echo "$cmdline" | tr ' ' '\n' | grep "$need=")"
  else
    emit_fail "Boot-Arg $need= fehlt"
  fi
done

# ---- 3) CPU-Governor ----
gov=$(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null || echo "?")
if [ "$gov" = "performance" ]; then
  emit_ok "CPU-Governor=performance"
else
  emit_fail "CPU-Governor=$gov (erwartet: performance)"
fi

# ---- 4) Hyper-Threading aus ----
ht_active=$(lscpu 2>/dev/null | awk -F: '/^Thread\(s\) per core/ {gsub(/^[ \t]+|[ \t]+$/,"",$2); print $2}' || echo "?")
if [ "$ht_active" = "1" ]; then
  emit_ok "Hyper-Threading aus (1 Thread per core)"
else
  emit_warn "Hyper-Threading aktiv (Threads/Core=$ht_active) — Cache-Sharing kann jittern"
fi

# ---- 5) C-States ----
if [ -d /sys/devices/system/cpu/cpu2/cpuidle ]; then
  for s in /sys/devices/system/cpu/cpu2/cpuidle/state*/disable; do
    name=$(cat "$(dirname "$s")/name")
    if [ "$name" != "POLL" ] && [ "$name" != "C1" ] && [ "$(cat "$s")" != "1" ]; then
      emit_warn "C-State $name auf cpu2 nicht disabled"
    fi
  done
fi

# ---- 6) NIC-Tuning (eth0) ----
if command -v ethtool >/dev/null; then
  rx_usecs=$(ethtool -c eth0 2>/dev/null | awk '/^rx-usecs:/ {print $2}' || echo "?")
  if [ "$rx_usecs" = "0" ]; then
    emit_ok "eth0 rx-usecs=0 (kein Coalescing)"
  else
    emit_warn "eth0 rx-usecs=$rx_usecs (erwartet 0, sonst Interrupt-Bursts)"
  fi
fi

# ---- 7) cyclictest verfuegbar ----
if command -v cyclictest >/dev/null; then
  emit_ok "cyclictest installiert: $(cyclictest -V 2>&1 | head -1)"
else
  emit_warn "cyclictest fehlt — install mit 'apt install rt-tests'"
fi

# ---- Summary ----
echo "---"
echo "checks: $bad fail, $warn warn"
if [ "$bad" -gt 0 ]; then
  echo "Host nicht RT-tauglich. Siehe tests/perf/rt-tuning/host-config-checklist.md"
  exit 1
fi
echo "OK — Host ist bench-bereit."
exit 0
