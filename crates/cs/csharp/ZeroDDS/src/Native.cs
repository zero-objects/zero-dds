// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Native.cs — P/Invoke for libzerodds (zerodds.h C-FFI).
// NativeAOT-compatible: all DllImport with explicit EntryPoint, no reflection.

using System;
using System.Runtime.InteropServices;

namespace ZeroDDS;

internal static class Native
{
    private const string LibName = "zerodds";

    // ====== Status-Codes (mirror ZeroDdsStatus) ======
    public const int Ok = 0;
    public const int Error = -1;
    public const int BadHandle = -2;
    public const int InvalidUtf8 = -3;
    public const int Timeout = -4;
    public const int PreconditionNotMet = -5;
    public const int BadParameter = -6;
    public const int NoData = -7;
    public const int OutOfResources = -8;
    public const int NotEnabled = -9;
    public const int ImmutablePolicy = -10;
    public const int InconsistentPolicy = -11;
    public const int AlreadyDeleted = -12;
    public const int Unsupported = -13;
    public const int IllegalOperation = -14;

    // ====== DomainParticipantFactory ======
    [DllImport(LibName, EntryPoint = "zerodds_dpf_get_instance")]
    public static extern IntPtr DpfGetInstance();

    [DllImport(LibName, EntryPoint = "zerodds_dpf_create_participant")]
    public static extern IntPtr DpfCreateParticipant(IntPtr factory, uint domainId, IntPtr qos);

    [DllImport(LibName, EntryPoint = "zerodds_dpf_delete_participant")]
    public static extern int DpfDeleteParticipant(IntPtr factory, IntPtr participant);

    [DllImport(LibName, EntryPoint = "zerodds_dpf_lookup_participant")]
    public static extern IntPtr DpfLookupParticipant(IntPtr factory, uint domainId);

    // ====== DomainParticipant ======
    [DllImport(LibName, EntryPoint = "zerodds_dp_create_topic", CharSet = CharSet.Ansi)]
    public static extern IntPtr DpCreateTopic(IntPtr p,
        [MarshalAs(UnmanagedType.LPStr)] string name,
        [MarshalAs(UnmanagedType.LPStr)] string typeName,
        IntPtr qos);

    [DllImport(LibName, EntryPoint = "zerodds_dp_delete_topic")]
    public static extern int DpDeleteTopic(IntPtr p, IntPtr topic);

    [DllImport(LibName, EntryPoint = "zerodds_dp_find_topic", CharSet = CharSet.Ansi)]
    public static extern IntPtr DpFindTopic(IntPtr p, [MarshalAs(UnmanagedType.LPStr)] string name);

    [DllImport(LibName, EntryPoint = "zerodds_dp_create_publisher")]
    public static extern IntPtr DpCreatePublisher(IntPtr p, IntPtr qos);

    [DllImport(LibName, EntryPoint = "zerodds_dp_delete_publisher")]
    public static extern int DpDeletePublisher(IntPtr p, IntPtr pub);

    [DllImport(LibName, EntryPoint = "zerodds_dp_create_subscriber")]
    public static extern IntPtr DpCreateSubscriber(IntPtr p, IntPtr qos);

    [DllImport(LibName, EntryPoint = "zerodds_dp_delete_subscriber")]
    public static extern int DpDeleteSubscriber(IntPtr p, IntPtr sub);

    [DllImport(LibName, EntryPoint = "zerodds_dp_get_domain_id")]
    public static extern uint DpGetDomainId(IntPtr p);

    [DllImport(LibName, EntryPoint = "zerodds_dp_assert_liveliness")]
    public static extern int DpAssertLiveliness(IntPtr p);

    [DllImport(LibName, EntryPoint = "zerodds_dp_contains_entity")]
    public static extern int DpContainsEntity(IntPtr p, ulong handle);

    [DllImport(LibName, EntryPoint = "zerodds_dp_ignore_participant")]
    public static extern int DpIgnoreParticipant(IntPtr p, ulong handle);

    [DllImport(LibName, EntryPoint = "zerodds_dp_delete_contained_entities")]
    public static extern int DpDeleteContainedEntities(IntPtr p);

    // ====== Topic ======
    [DllImport(LibName, EntryPoint = "zerodds_topic_get_name")]
    public static extern IntPtr TopicGetName(IntPtr topic);

    [DllImport(LibName, EntryPoint = "zerodds_topic_get_type_name")]
    public static extern IntPtr TopicGetTypeName(IntPtr topic);

    [DllImport(LibName, EntryPoint = "zerodds_string_free")]
    public static extern void StringFree(IntPtr s);

    // ====== Publisher / DataWriter ======
    [DllImport(LibName, EntryPoint = "zerodds_pub_create_datawriter")]
    public static extern IntPtr PubCreateDatawriter(IntPtr pub, IntPtr topic, IntPtr qos);

    [DllImport(LibName, EntryPoint = "zerodds_pub_delete_datawriter")]
    public static extern int PubDeleteDatawriter(IntPtr pub, IntPtr dw);

    [DllImport(LibName, EntryPoint = "zerodds_pub_lookup_datawriter", CharSet = CharSet.Ansi)]
    public static extern IntPtr PubLookupDatawriter(IntPtr pub,
        [MarshalAs(UnmanagedType.LPStr)] string topicName);

    [DllImport(LibName, EntryPoint = "zerodds_pub_suspend_publications")]
    public static extern int PubSuspendPublications(IntPtr pub);

    [DllImport(LibName, EntryPoint = "zerodds_pub_resume_publications")]
    public static extern int PubResumePublications(IntPtr pub);

    [DllImport(LibName, EntryPoint = "zerodds_pub_wait_for_acknowledgments")]
    public static extern int PubWaitForAcks(IntPtr pub, ulong timeoutMs);

    [DllImport(LibName, EntryPoint = "zerodds_dw_write")]
    public static extern int DwWrite(IntPtr dw, IntPtr payload, UIntPtr len, ulong handle);

    [DllImport(LibName, EntryPoint = "zerodds_dw_dispose")]
    public static extern int DwDispose(IntPtr dw, IntPtr keyHash, ulong handle);

    [DllImport(LibName, EntryPoint = "zerodds_dw_dispose_w_timestamp")]
    public static extern int DwDisposeWTimestamp(IntPtr dw, IntPtr keyHash, ulong handle,
        int tsSec, uint tsNanosec);

    [DllImport(LibName, EntryPoint = "zerodds_dw_register_instance")]
    public static extern int DwRegisterInstance(IntPtr dw, IntPtr key, UIntPtr keyLen,
        out ulong outHandle);

    [DllImport(LibName, EntryPoint = "zerodds_dw_register_instance_w_timestamp")]
    public static extern int DwRegisterInstanceWTimestamp(IntPtr dw, IntPtr key, UIntPtr keyLen,
        int tsSec, uint tsNanosec, out ulong outHandle);

    [DllImport(LibName, EntryPoint = "zerodds_dw_unregister_instance")]
    public static extern int DwUnregisterInstance(IntPtr dw, ulong handle);

    [DllImport(LibName, EntryPoint = "zerodds_dw_unregister_instance_w_timestamp")]
    public static extern int DwUnregisterInstanceWTimestamp(IntPtr dw, ulong handle,
        int tsSec, uint tsNanosec);

    [DllImport(LibName, EntryPoint = "zerodds_dw_lookup_instance")]
    public static extern int DwLookupInstance(IntPtr dw, IntPtr key, UIntPtr keyLen,
        out ulong outHandle);

    [DllImport(LibName, EntryPoint = "zerodds_dr_lookup_instance")]
    public static extern int DrLookupInstance(IntPtr dr, IntPtr key, UIntPtr keyLen,
        out ulong outHandle);

    [DllImport(LibName, EntryPoint = "zerodds_dw_wait_for_acknowledgments")]
    public static extern int DwWaitForAcks(IntPtr dw, ulong timeoutMs);

    [DllImport(LibName, EntryPoint = "zerodds_dw_assert_liveliness")]
    public static extern int DwAssertLiveliness(IntPtr dw);

    [DllImport(LibName, EntryPoint = "zerodds_dw_wait_for_matched")]
    public static extern int DwWaitForMatched(IntPtr dw, int min, ulong timeoutMs);

    [StructLayout(LayoutKind.Sequential)]
    public struct PublicationMatchedStatus
    {
        public int TotalCount;
        public int TotalCountChange;
        public int CurrentCount;
        public int CurrentCountChange;
        public ulong LastSubscriptionHandle;
    }

    [DllImport(LibName, EntryPoint = "zerodds_dw_get_publication_matched_status")]
    public static extern int DwGetPublicationMatchedStatus(IntPtr dw,
        out PublicationMatchedStatus s);

    // ====== Subscriber / DataReader ======
    [DllImport(LibName, EntryPoint = "zerodds_sub_create_datareader")]
    public static extern IntPtr SubCreateDatareader(IntPtr sub, IntPtr topic, IntPtr qos);

    [DllImport(LibName, EntryPoint = "zerodds_sub_delete_datareader")]
    public static extern int SubDeleteDatareader(IntPtr sub, IntPtr dr);

    [DllImport(LibName, EntryPoint = "zerodds_sub_lookup_datareader", CharSet = CharSet.Ansi)]
    public static extern IntPtr SubLookupDatareader(IntPtr sub,
        [MarshalAs(UnmanagedType.LPStr)] string topicName);

    [StructLayout(LayoutKind.Sequential)]
    public struct SampleInfoNative
    {
        public uint SampleState;
        public uint ViewState;
        public uint InstanceState;
        public int DisposedGenerationCount;
        public int NoWritersGenerationCount;
        public int SampleRank;
        public int GenerationRank;
        public int AbsoluteGenerationRank;
        public int SourceTimestampSec;
        public uint SourceTimestampNanosec;
        public ulong InstanceHandle;
        public ulong PublicationHandle;
        [MarshalAs(UnmanagedType.U1)]
        public bool ValidData;
        // XCDR representation (0 = XCDR1, 1 = XCDR2) and wire byte order
        // (0 = little-endian, 1 = big-endian) read from the encapsulation
        // header, so the typed decoder picks Decode vs the big-endian path.
        public byte Representation;
        public byte BigEndian;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct SampleArray
    {
        public IntPtr Buffers;
        public IntPtr Lengths;
        public IntPtr Infos;
        public UIntPtr Count;
        public IntPtr LoanToken;
    }

    [DllImport(LibName, EntryPoint = "zerodds_dr_take")]
    public static extern int DrTake(IntPtr dr, ref SampleArray arr, UIntPtr maxSamples,
        uint sampleStates, uint viewStates, uint instanceStates);

    [DllImport(LibName, EntryPoint = "zerodds_dr_return_loan")]
    public static extern int DrReturnLoan(IntPtr dr, ref SampleArray arr);

    [DllImport(LibName, EntryPoint = "zerodds_dr_wait_for_matched")]
    public static extern int DrWaitForMatched(IntPtr dr, int min, ulong timeoutMs);

    [StructLayout(LayoutKind.Sequential)]
    public struct SubscriptionMatchedStatus
    {
        public int TotalCount;
        public int TotalCountChange;
        public int CurrentCount;
        public int CurrentCountChange;
        public ulong LastPublicationHandle;
    }

    [DllImport(LibName, EntryPoint = "zerodds_dr_get_subscription_matched_status")]
    public static extern int DrGetSubscriptionMatchedStatus(IntPtr dr,
        out SubscriptionMatchedStatus s);

    [StructLayout(LayoutKind.Sequential)]
    public struct SampleLostStatus
    {
        public int TotalCount;
        public int TotalCountChange;
    }

    [DllImport(LibName, EntryPoint = "zerodds_dr_get_sample_lost_status")]
    public static extern int DrGetSampleLostStatus(IntPtr dr, out SampleLostStatus s);

    [StructLayout(LayoutKind.Sequential)]
    public struct LivelinessChangedStatus
    {
        public int AliveCount;
        public int NotAliveCount;
        public int AliveCountChange;
        public int NotAliveCountChange;
        public ulong LastPublicationHandle;
    }

    [DllImport(LibName, EntryPoint = "zerodds_dr_get_liveliness_changed_status")]
    public static extern int DrGetLivelinessChangedStatus(IntPtr dr, out LivelinessChangedStatus s);

    [StructLayout(LayoutKind.Sequential)]
    public struct RequestedDeadlineMissedStatus
    {
        public int TotalCount;
        public int TotalCountChange;
        public ulong LastInstanceHandle;
    }

    [DllImport(LibName, EntryPoint = "zerodds_dr_get_requested_deadline_missed_status")]
    public static extern int DrGetRequestedDeadlineMissedStatus(IntPtr dr,
        out RequestedDeadlineMissedStatus s);

    [StructLayout(LayoutKind.Sequential)]
    public struct RequestedIncompatibleQosStatus
    {
        public int TotalCount;
        public int TotalCountChange;
        public uint LastPolicyId;
    }

    [DllImport(LibName, EntryPoint = "zerodds_dr_get_requested_incompatible_qos_status")]
    public static extern int DrGetRequestedIncompatibleQosStatus(IntPtr dr,
        out RequestedIncompatibleQosStatus s);

    [StructLayout(LayoutKind.Sequential)]
    public struct SampleRejectedStatus
    {
        public int TotalCount;
        public int TotalCountChange;
        public uint LastReason;
        public ulong LastInstanceHandle;
    }

    [DllImport(LibName, EntryPoint = "zerodds_dr_get_sample_rejected_status")]
    public static extern int DrGetSampleRejectedStatus(IntPtr dr, out SampleRejectedStatus s);

    [StructLayout(LayoutKind.Sequential)]
    public struct LivelinessLostStatus
    {
        public int TotalCount;
        public int TotalCountChange;
    }

    [DllImport(LibName, EntryPoint = "zerodds_dw_get_liveliness_lost_status")]
    public static extern int DwGetLivelinessLostStatus(IntPtr dw, out LivelinessLostStatus s);

    [StructLayout(LayoutKind.Sequential)]
    public struct OfferedDeadlineMissedStatus
    {
        public int TotalCount;
        public int TotalCountChange;
        public ulong LastInstanceHandle;
    }

    [DllImport(LibName, EntryPoint = "zerodds_dw_get_offered_deadline_missed_status")]
    public static extern int DwGetOfferedDeadlineMissedStatus(IntPtr dw,
        out OfferedDeadlineMissedStatus s);

    [StructLayout(LayoutKind.Sequential)]
    public struct OfferedIncompatibleQosStatus
    {
        public int TotalCount;
        public int TotalCountChange;
        public uint LastPolicyId;
    }

    [DllImport(LibName, EntryPoint = "zerodds_dw_get_offered_incompatible_qos_status")]
    public static extern int DwGetOfferedIncompatibleQosStatus(IntPtr dw,
        out OfferedIncompatibleQosStatus s);

    // ====== ReadCondition / QueryCondition ======
    [DllImport(LibName, EntryPoint = "zerodds_dr_create_readcondition")]
    public static extern IntPtr DrCreateReadCondition(IntPtr dr, uint sampleStates,
        uint viewStates, uint instanceStates);

    [DllImport(LibName, EntryPoint = "zerodds_dr_delete_readcondition")]
    public static extern int DrDeleteReadCondition(IntPtr dr, IntPtr cond);

    [DllImport(LibName, EntryPoint = "zerodds_dr_create_querycondition", CharSet = CharSet.Ansi)]
    public static extern IntPtr DrCreateQueryCondition(IntPtr dr, uint sampleStates,
        uint viewStates, uint instanceStates,
        [MarshalAs(UnmanagedType.LPStr)] string expr,
        IntPtr parameters, UIntPtr paramCount);

    // ====== Listener vtable structs ======
    [StructLayout(LayoutKind.Sequential)]
    public struct DataWriterListenerVTable
    {
        public IntPtr UserData;
        public IntPtr OnLivelinessLost;
        public IntPtr OnOfferedDeadlineMissed;
        public IntPtr OnOfferedIncompatibleQos;
        public IntPtr OnPublicationMatched;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct DataReaderListenerVTable
    {
        public IntPtr UserData;
        public IntPtr OnDataAvailable;
        public IntPtr OnSampleRejected;
        public IntPtr OnLivelinessChanged;
        public IntPtr OnRequestedDeadlineMissed;
        public IntPtr OnRequestedIncompatibleQos;
        public IntPtr OnSubscriptionMatched;
        public IntPtr OnSampleLost;
    }

    [DllImport(LibName, EntryPoint = "zerodds_dw_set_listener")]
    public static extern int DwSetListener(IntPtr dw,
        ref DataWriterListenerVTable listener, uint statusMask);

    [DllImport(LibName, EntryPoint = "zerodds_dw_set_listener")]
    public static extern int DwSetListenerNull(IntPtr dw, IntPtr listener, uint statusMask);

    [DllImport(LibName, EntryPoint = "zerodds_dr_set_listener")]
    public static extern int DrSetListener(IntPtr dr,
        ref DataReaderListenerVTable listener, uint statusMask);

    [DllImport(LibName, EntryPoint = "zerodds_dr_set_listener")]
    public static extern int DrSetListenerNull(IntPtr dr, IntPtr listener, uint statusMask);

    [DllImport(LibName, EntryPoint = "zerodds_poll_listeners")]
    public static extern UIntPtr PollListeners();

    // ====== Conditions / WaitSet ======
    [DllImport(LibName, EntryPoint = "zerodds_guardcondition_create")]
    public static extern IntPtr GuardConditionCreate();

    [DllImport(LibName, EntryPoint = "zerodds_guardcondition_destroy")]
    public static extern void GuardConditionDestroy(IntPtr g);

    [DllImport(LibName, EntryPoint = "zerodds_guardcondition_set_trigger_value")]
    public static extern int GuardConditionSetTrigger(IntPtr g, [MarshalAs(UnmanagedType.U1)] bool v);

    [DllImport(LibName, EntryPoint = "zerodds_condition_get_trigger_value")]
    [return: MarshalAs(UnmanagedType.U1)]
    public static extern bool ConditionGetTrigger(IntPtr c);

    [DllImport(LibName, EntryPoint = "zerodds_waitset_create")]
    public static extern IntPtr WaitSetCreate();

    [DllImport(LibName, EntryPoint = "zerodds_waitset_destroy")]
    public static extern void WaitSetDestroy(IntPtr w);

    [DllImport(LibName, EntryPoint = "zerodds_waitset_attach_condition")]
    public static extern int WaitSetAttach(IntPtr w, IntPtr c);

    [DllImport(LibName, EntryPoint = "zerodds_waitset_detach_condition")]
    public static extern int WaitSetDetach(IntPtr w, IntPtr c);

    [DllImport(LibName, EntryPoint = "zerodds_waitset_wait")]
    public static extern int WaitSetWait(IntPtr w, IntPtr outActive, UIntPtr cap,
        out UIntPtr outCount, int sec, uint nanosec);

    // ====== ContentFilteredTopic (Spec §2.2.2.3.3) ======
    [DllImport(LibName, EntryPoint = "zerodds_dp_create_contentfilteredtopic", CharSet = CharSet.Ansi)]
    public static extern IntPtr DpCreateContentFilteredTopic(IntPtr p,
        [MarshalAs(UnmanagedType.LPStr)] string name,
        IntPtr related,
        [MarshalAs(UnmanagedType.LPStr)] string filterExpression,
        IntPtr parameters, UIntPtr paramCount);

    [DllImport(LibName, EntryPoint = "zerodds_cft_set_schema")]
    public static extern int CftSetSchema(IntPtr cft, IntPtr names, IntPtr kinds, UIntPtr count);

    [DllImport(LibName, EntryPoint = "zerodds_dp_delete_contentfilteredtopic")]
    public static extern int DpDeleteContentFilteredTopic(IntPtr p, IntPtr cft);

    [DllImport(LibName, EntryPoint = "zerodds_sub_create_datareader_with_cft")]
    public static extern IntPtr SubCreateDataReaderWithCft(IntPtr sub, IntPtr cft, IntPtr qos);
}
