// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Zerodds.Rpc — C# DDS-RPC runtime contract (DDS-RPC 1.0 §7.11.2, C# PSM).
//
// Generated `<Service>Requester` / `<Service>Replier` classes depend on these
// types. This file is the C# counterpart of `crates/idl-java/runtime/rpc/*`.
// It declares the API surface only; a transport binding provides the concrete
// IRequester / IReplier. The in-process marshalling round-trip test wires a
// trivial implementation to prove the generated code actually marshals.

using System;
using System.Threading.Tasks;

namespace Zerodds.Rpc
{
    /// <summary>Marks an interface as a DDS-RPC service (§7.11.2).</summary>
    [AttributeUsage(AttributeTargets.Interface, AllowMultiple = false)]
    public sealed class ServiceAttribute : Attribute
    {
        public string Name { get; }
        public ServiceAttribute(string name) { Name = name; }
    }

    /// <summary>Marks a method as fire-and-forget (no reply expected).</summary>
    [AttributeUsage(AttributeTargets.Method, AllowMultiple = false)]
    public sealed class OnewayAttribute : Attribute { }

    /// <summary>
    /// Mutable single-field holder for IDL <c>out</c> / <c>inout</c> parameters
    /// (DDS-RPC 1.0 §7.11.2.3). The caller pre-allocates it; the callee (or the
    /// runtime, after decoding the reply) writes to <see cref="Value"/>.
    /// </summary>
    public sealed class Holder<T>
    {
        public T Value;
        public Holder() { Value = default!; }
        public Holder(T value) { Value = value; }
    }

    /// <summary>Wire-encoded RPC error code (DDS-RPC 1.0 §7.5.1.1.1).</summary>
    public enum RemoteExceptionCode
    {
        Ok = 0,
        Unsupported = 1,
        InvalidArgument = 2,
        OutOfResources = 3,
        UnknownOperation = 4,
        UnknownException = 5,
        UnknownInterface = 6,
    }

    /// <summary>Transport- / dispatch-level RPC failure (DDS-RPC 1.0 §7.11.2.1).</summary>
    public class RemoteException : Exception
    {
        public RemoteExceptionCode ErrorCode { get; }

        public RemoteException(string message, RemoteExceptionCode code)
            : base(message)
        {
            ErrorCode = code;
        }

        public RemoteException(Exception cause)
            : base(cause?.Message, cause)
        {
            ErrorCode = RemoteExceptionCode.UnknownException;
        }
    }

    /// <summary>
    /// Client-side endpoint. A generated Requester marshals a per-method
    /// <c>object[]</c> tuple and calls <see cref="SendRequest"/> /
    /// <see cref="SendOneway"/>; the reply is the server's reply tuple.
    /// </summary>
    public interface IRequester : IDisposable
    {
        /// <summary>Sends a request and completes with the reply tuple.</summary>
        Task<object> SendRequest(int methodId, object payload);

        /// <summary>Sends a fire-and-forget oneway request.</summary>
        void SendOneway(int methodId, object payload);
    }

    /// <summary>
    /// Server-side endpoint. A generated Replier holds one and routes decoded
    /// requests to the service handler via its <c>Dispatch</c> method.
    /// </summary>
    public interface IReplier : IDisposable
    {
        /// <summary>Sends a reply tuple for a previously received request.</summary>
        void SendReply(object payload);
    }
}
