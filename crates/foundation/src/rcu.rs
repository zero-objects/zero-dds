// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! RCU-Cell — Copy-on-Write-Container fuer wenig-Schreib/viel-Lese-Patterns.
//!
//! Foundation-Primitive fuer einen Lock-Free-Read-History-Cache.
//! Reader greifen auf einen `Arc<T>`-Snapshot zu, der von
//! einem kurzen Mutex-Aufruf (Microsekunden) geliefert wird; danach lebt
//! der Snapshot referenzgezaehlt, **ohne** weiteren Lock-Touch.
//!
//! # Designziel
//!
//! Klassischer RCU mit `AtomicPtr`-Pointer-Swap braucht `unsafe`
//! (Pointer-Provenance, Memory-Ordering). Diese Implementation tradet
//! mikroskopische Performance gegen Strikt-Safe-Code: ein `Mutex<Arc<T>>`
//! schuetzt nur die Reference-Cell, **nicht** den Inhalt — der Inhalt
//! wird geklont, bevor der Lock faellt.
//!
//! Performance-Profil:
//!
//! * **Reader**: 1× Mutex acquire/release + 1× Arc-Clone (refcount inc).
//!   Mehrere Reader **serialisieren** kurz auf dem Cell-Mutex — jeder
//!   haelt ihn nur fuer einen Refcount-Inc, also Sub-Microsekunde.
//! * **Writer**: 1× Mutex acquire + Copy-on-Write von `T` (= Allocation
//!   einer neuen Inner-Struktur via Closure) + Arc-Replace + Mutex
//!   release.
//! * **Snapshot-Lebensdauer**: nach Reader-Read existiert die `Arc<T>`-
//!   Snapshot unabhaengig vom Cell-Mutex. Reader und Writer interferieren
//!   nicht mehr.
//!
//! Fuer einen echten `AtomicPtr`-Swap (lock-frei beim Read) waere
//! `arc-swap` als externe Crate die Standard-Wahl. Wir vermeiden das,
//! solange std-Mittel ausreichen.
//!
//! # Anwendung in ZeroDDS
//!
//! `zerodds_rtps::history_cache::LockFreeReadHistoryCache` (D.4 Phase C-2)
//! nutzt diese Cell als Backing-Store. Monitoring/Tick-Loops koennen
//! dann Snapshots ziehen, ohne den Writer-Lock zu nehmen — der eigentliche
//! Insert-Pfad bleibt synchron unter dem Per-Slot-Mutex aus D.4 Phase B.

#[cfg(feature = "alloc")]
use alloc::sync::Arc;

#[cfg(feature = "std")]
use std::sync::Mutex;

/// RCU-Cell: read-only-Snapshot via Arc-Clone, write via Copy-on-Write.
///
/// Generisch ueber `T`. `T` muss `Clone` sein — der Writer-Pfad
/// kopiert den aktuellen Stand und uebergibt eine `&T` an die Mutator-
/// Closure, die einen neuen `T` zurueckgibt.
///
/// ```
/// use zerodds_foundation::rcu::RcuCell;
///
/// let cell = RcuCell::new(0u32);
/// let r = cell.read();
/// assert_eq!(*r, 0);
/// cell.write_with(|cur| cur + 1);
/// assert_eq!(*cell.read(), 1);
/// // Der alte Snapshot ist unveraendert:
/// assert_eq!(*r, 0);
/// ```
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct RcuCell<T> {
    inner: Mutex<Arc<T>>,
}

#[cfg(feature = "std")]
impl<T> RcuCell<T> {
    /// Erzeugt eine neue Cell mit Initialwert.
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            inner: Mutex::new(Arc::new(value)),
        }
    }

    /// Liefert einen Read-Snapshot. Der Mutex wird **nur** fuer den
    /// `Arc::clone()` gehalten — danach ist der zurueckgegebene Arc
    /// unabhaengig von der Cell.
    ///
    /// # Panics
    /// Niemals — Mutex-Poisoning wird via `into_inner` umgangen.
    #[must_use]
    pub fn read(&self) -> Arc<T> {
        match self.inner.lock() {
            Ok(g) => Arc::clone(&g),
            // Bei Poisoning den letzten validen Stand zurueckgeben — die
            // Cell ist read-only erreichbar, weil Inner durch `Arc` und
            // damit `Clone` strukturiert ist.
            Err(p) => Arc::clone(&p.into_inner()),
        }
    }

    /// Aktualisiert den Inhalt copy-on-write. Die Closure bekommt eine
    /// Referenz auf den aktuellen Stand und gibt den neuen Stand
    /// zurueck. Der Writer haelt den Mutex nur fuer Closure-Lauf +
    /// Arc-Replace.
    pub fn write_with(&self, f: impl FnOnce(&T) -> T)
    where
        T: Clone,
    {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let new = f(&guard);
        *guard = Arc::new(new);
    }

    /// In-place-Mutation auf einer **lokalen Kopie** des Inhalts.
    /// Sequenz: Snapshot ziehen, klonen, Closure ruft `&mut`-Mutator
    /// auf der Kopie, dann Arc-Replace.
    pub fn modify(&self, f: impl FnOnce(&mut T))
    where
        T: Clone,
    {
        self.write_with(|cur| {
            let mut new = cur.clone();
            f(&mut new);
            new
        });
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec::Vec;

    #[test]
    fn new_cell_returns_initial_value() {
        let cell = RcuCell::new(42u32);
        assert_eq!(*cell.read(), 42);
    }

    #[test]
    fn write_with_replaces_value() {
        let cell = RcuCell::new(1u32);
        cell.write_with(|c| c + 10);
        assert_eq!(*cell.read(), 11);
    }

    #[test]
    fn old_snapshot_is_immutable_after_write() {
        let cell = RcuCell::new(1u32);
        let snap = cell.read();
        cell.write_with(|c| c + 100);
        assert_eq!(*snap, 1, "RCU snapshot is decoupled from later writes");
        assert_eq!(*cell.read(), 101);
    }

    #[test]
    fn multiple_readers_share_arc() {
        let cell = RcuCell::new("hello".to_string());
        let a = cell.read();
        let b = cell.read();
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn modify_uses_clone_and_mutator() {
        let cell = RcuCell::new(alloc::vec![1u32, 2, 3]);
        cell.modify(|v| v.push(4));
        let r = cell.read();
        assert_eq!(*r, alloc::vec![1, 2, 3, 4]);
    }

    #[test]
    fn concurrent_readers_writers_smoke() {
        // 1 Writer, 4 Reader, 10ms — kein Deadlock, keine Daten-Verluste.
        use std::sync::Arc as StdArc;
        use std::thread;
        use std::time::Duration;

        let cell: StdArc<RcuCell<u64>> = StdArc::new(RcuCell::new(0));
        let stop: StdArc<std::sync::atomic::AtomicBool> =
            StdArc::new(std::sync::atomic::AtomicBool::new(false));

        let writer = {
            let cell = StdArc::clone(&cell);
            let stop = StdArc::clone(&stop);
            thread::spawn(move || {
                let mut i = 0u64;
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    cell.write_with(|_| i);
                    i = i.wrapping_add(1);
                }
                i
            })
        };
        let mut readers = Vec::new();
        for _ in 0..4 {
            let cell = StdArc::clone(&cell);
            let stop = StdArc::clone(&stop);
            readers.push(thread::spawn(move || {
                let mut last = 0u64;
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    let v = *cell.read();
                    // Wir koennen nur garantieren dass Werte monoton sind;
                    // u64-wrap nach 1e10 Iterationen ist nicht in 10ms zu
                    // sehen, also strikt monoton.
                    assert!(v >= last);
                    last = v;
                }
                last
            }));
        }
        thread::sleep(Duration::from_millis(10));
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let writer_count = writer.join().unwrap();
        for r in readers {
            let last = r.join().unwrap();
            assert!(last <= writer_count);
        }
    }

    #[test]
    fn write_with_recovers_from_poisoned_mutex() {
        // Wenn ein Writer mit Panic den Mutex vergiftet, soll der naechste
        // Reader/Writer trotzdem den letzten validen Stand sehen.
        use std::sync::Arc as StdArc;
        let cell: StdArc<RcuCell<u32>> = StdArc::new(RcuCell::new(7));
        let cell_p = StdArc::clone(&cell);
        // Panic im Writer-Pfad: closure panic'd, mutex bleibt vergiftet.
        let _ = std::thread::spawn(move || {
            cell_p.write_with(|_| panic!("intentional"));
        })
        .join();
        // read() darf nicht selbst panicen + liefert den letzten validen
        // Wert (7), weil die Closure den Mutex erst NACH ihrer Ausfuehrung
        // veroeffentlicht haette.
        assert_eq!(*cell.read(), 7);
        // write_with auf vergiftetem Mutex — recovers.
        cell.write_with(|c| c + 1);
        assert_eq!(*cell.read(), 8);
    }
}
