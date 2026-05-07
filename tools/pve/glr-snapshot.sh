#!/usr/bin/env bash
# glr-snapshot.sh — PVE-Snapshot-Helper fuer GitLab-Runner-VMs.
#
# Usage:
#   glr-snapshot.sh list <vmid>
#   glr-snapshot.sh rollback <vmid> <snapshot>
#   glr-snapshot.sh save <vmid> <name> [description]
#   glr-snapshot.sh delete <vmid> <name>
#
# Auf pve als root ausfuehren. Loggt jede Aktion + macht
# qm-shutdown sauber statt hard-stop wenn moeglich.

set -euo pipefail

CMD="${1:-help}"
shift || true

usage() {
    sed -n '1,/^set -e/p' "$0" | head -n -2 | sed 's/^# \?//'
    exit 0
}

require_vmid() {
    local vmid="${1:?vmid fehlt}"
    if ! qm list 2>/dev/null | awk '{print $1}' | grep -qx "$vmid"; then
        echo "FEHLER: VM $vmid existiert nicht auf diesem Node." >&2
        echo "Verfuegbare VMs:" >&2
        qm list >&2
        exit 1
    fi
}

is_running() {
    qm status "$1" 2>/dev/null | grep -q running
}

graceful_stop() {
    local vmid="$1"
    if is_running "$vmid"; then
        echo "→ shutdown VM $vmid (timeout 60s)..."
        if ! qm shutdown "$vmid" --timeout 60 >/dev/null 2>&1; then
            echo "  graceful shutdown timeout, hard stop..."
            qm stop "$vmid"
        fi
    fi
}

case "$CMD" in
    list)
        require_vmid "${1:-}"
        echo "Snapshots fuer VM $1:"
        qm listsnapshot "$1"
        ;;
    save)
        VMID="${1:?vmid fehlt}"; SNAP="${2:?snapshot-name fehlt}"
        DESC="${3:-saved $(date -Iseconds) by $USER}"
        require_vmid "$VMID"
        echo "Snapshot '$SNAP' fuer VM $VMID anlegen..."
        qm snapshot "$VMID" "$SNAP" --description "$DESC"
        echo "✓ Snapshot '$SNAP' angelegt."
        ;;
    rollback)
        VMID="${1:?vmid fehlt}"; SNAP="${2:?snapshot-name fehlt}"
        require_vmid "$VMID"
        if ! qm listsnapshot "$VMID" | awk '{print $2}' | grep -qx "$SNAP"; then
            echo "FEHLER: Snapshot '$SNAP' existiert nicht fuer VM $VMID." >&2
            qm listsnapshot "$VMID" >&2
            exit 1
        fi
        graceful_stop "$VMID"
        echo "→ rollback VM $VMID auf '$SNAP'..."
        qm rollback "$VMID" "$SNAP"
        echo "→ VM $VMID starten..."
        qm start "$VMID"
        echo "✓ VM $VMID jetzt auf snapshot '$SNAP' und am Laufen."
        ;;
    delete)
        VMID="${1:?vmid fehlt}"; SNAP="${2:?snapshot-name fehlt}"
        require_vmid "$VMID"
        echo "Snapshot '$SNAP' fuer VM $VMID loeschen..."
        qm delsnapshot "$VMID" "$SNAP"
        echo "✓ Snapshot '$SNAP' geloescht."
        ;;
    help|-h|--help|"")
        usage
        ;;
    *)
        echo "Unbekanntes Kommando: $CMD" >&2
        usage
        ;;
esac
