// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
using System;

namespace ZeroDds.Idl
{
    /// <summary>
    /// The IDL spelling of the declaration it annotates.
    /// </summary>
    /// <remarks>
    /// <para>
    /// Load-bearing, not documentation. A member's ID under
    /// <c>@autoid(HASH)</c> and under <c>@hashid</c> is the first four octets
    /// of the MD5 of its IDL name (XTypes 1.3 §7.3.1.2.1.1), so a field the
    /// mapping renamed to <c>PascalCase</c> cannot compute its own ID without
    /// this.
    /// </para>
    /// <para>
    /// The same declaration <c>crates/lang-rules/src/adapter/csharp.rs</c>
    /// emits into a standalone generated file. It is here as well so that a
    /// generated file compiled <i>with</i> this runtime can drop its own copy
    /// rather than declare a second type of the same name (CS0101).
    /// <c>runtime/csharp/tests/SelfTest.cs</c> holds the two together.
    /// </para>
    /// </remarks>
    [AttributeUsage(
        AttributeTargets.Class | AttributeTargets.Struct | AttributeTargets.Enum
        | AttributeTargets.Field | AttributeTargets.Method | AttributeTargets.Interface
        | AttributeTargets.Property,
        AllowMultiple = false,
        Inherited = false)]
    public sealed class IdlNameAttribute : Attribute
    {
        /// <summary>The identifier as the IDL source spelled it.</summary>
        public IdlNameAttribute(string name)
        {
            Name = name;
        }

        /// <summary>The identifier as the IDL source spelled it.</summary>
        public string Name { get; private set; }
    }
}
