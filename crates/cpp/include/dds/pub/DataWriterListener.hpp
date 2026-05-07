// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/DataWriterListener.hpp — DDS-PSM-Cxx 1.0 §7.5.9.5.

#ifndef ZERODDS_DDS_PUB_DATAWRITERLISTENER_HPP
#define ZERODDS_DDS_PUB_DATAWRITERLISTENER_HPP

#include "dds/core/Status.hpp"
#include "dds/pub/Publisher.hpp"
#include "zerodds.h"

namespace dds {
namespace pub {

/// `DataWriterListener<T>` (Spec §7.5.9.5).
///
/// User-Code abgeleitet, ueberschreibt die On-XXX-Methoden. Wird per
/// `dds::pub::DataWriter<T>::set_listener(...)` registriert.
///
/// Active-Wireup an die Runtime erfolgt ueber den C-FFI-Pfad
/// `zerodds_poll_listeners()` — Caller im Main-Loop pollen, dann
/// feuern die `on_*`-Methoden auf dem Caller-Thread (siehe
/// Vendor-Spec `docs/specs/zerodds-listener-callbacks-1.0.md` §6.2).
template <typename T>
class DataWriterListener {
public:
    virtual ~DataWriterListener() = default;
    /// Liveliness-Lost (Spec §2.2.4.2.5.1).
    virtual void on_liveliness_lost(DataWriter<T>& /*dw*/,
                                     const ::dds::core::status::LivelinessLostStatus& /*s*/) {}
    /// Offered-Deadline-Missed.
    virtual void on_offered_deadline_missed(
        DataWriter<T>& /*dw*/,
        const ::dds::core::status::OfferedDeadlineMissedStatus& /*s*/) {}
    /// Offered-Incompatible-QoS.
    virtual void on_offered_incompatible_qos(
        DataWriter<T>& /*dw*/,
        const ::dds::core::status::OfferedIncompatibleQosStatus& /*s*/) {}
    /// Publication-Matched.
    virtual void on_publication_matched(
        DataWriter<T>& /*dw*/,
        const ::dds::core::status::PublicationMatchedStatus& /*s*/) {}
};

/// Internal: ein Adapter der Spec-Listener auf die C-FFI-vtable mappt.
///
/// Caller-Code instantiiert eine konkrete `DataWriterListener<T>`-
/// Subclass und registriert sie via:
///
/// ```cpp
/// MyListener<MsgT> mine;
/// dw.set_listener(mine, dds::core::status::StatusMask::all());
/// ```
template <typename T>
class _DataWriterListenerBridge {
public:
    /// Bindet eine Listener-Subclass an einen DataWriter.
    static void attach(DataWriter<T>& dw, DataWriterListener<T>* listener,
                       uint32_t status_mask) {
        if (listener == nullptr) {
            zerodds_dw_set_listener(dw.native_handle(), nullptr, 0);
            return;
        }
        // Statisches Singleton fuer den vtable-Adapter — eine Instanz
        // pro Sprach-Binding-Typ. user_data zeigt auf die Listener-
        // Subclass; die C-FFI feuert die untenstehenden static-Funktionen.
        thread_local zerodds_ZeroDdsDataWriterListener vt;
        vt.user_data = static_cast<void*>(listener);
        vt.on_liveliness_lost = &cb_liveliness_lost;
        vt.on_offered_deadline_missed = &cb_offered_deadline_missed;
        vt.on_offered_incompatible_qos = &cb_offered_incompatible_qos;
        vt.on_publication_matched = &cb_publication_matched;
        zerodds_dw_set_listener(dw.native_handle(), &vt, status_mask);
    }

private:
    static DataWriterListener<T>* unwrap(void* user_data) {
        return static_cast<DataWriterListener<T>*>(user_data);
    }
    static void cb_liveliness_lost(void* ud, zerodds_ZeroDdsDataWriter*) {
        if (auto l = unwrap(ud)) {
            DataWriter<T> dummy;
            ::dds::core::status::LivelinessLostStatus s;
            l->on_liveliness_lost(dummy, s);
        }
    }
    static void cb_offered_deadline_missed(void* ud, zerodds_ZeroDdsDataWriter*) {
        if (auto l = unwrap(ud)) {
            DataWriter<T> dummy;
            ::dds::core::status::OfferedDeadlineMissedStatus s;
            l->on_offered_deadline_missed(dummy, s);
        }
    }
    static void cb_offered_incompatible_qos(void* ud, zerodds_ZeroDdsDataWriter*) {
        if (auto l = unwrap(ud)) {
            DataWriter<T> dummy;
            ::dds::core::status::OfferedIncompatibleQosStatus s;
            l->on_offered_incompatible_qos(dummy, s);
        }
    }
    static void cb_publication_matched(void* ud, zerodds_ZeroDdsDataWriter*) {
        if (auto l = unwrap(ud)) {
            DataWriter<T> dummy;
            ::dds::core::status::PublicationMatchedStatus s;
            l->on_publication_matched(dummy, s);
        }
    }
};

} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_DATAWRITERLISTENER_HPP
