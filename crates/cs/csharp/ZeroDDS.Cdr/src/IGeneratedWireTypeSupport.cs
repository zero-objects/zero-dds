// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
namespace ZeroDDS.Cdr;

/// <summary>
/// A generated codec's explicit wire-version capability. Entity creation must
/// configure the native endpoint to this representation, rather than inheriting
/// an unrelated default. The IDdsTopicType supplies its extensibility separately.
/// </summary>
public interface IGeneratedWireTypeSupport
{
    /// <summary>0 is XCDR1, 1 is XCDR2. No other value is supported.</summary>
    int Representation { get; }
}
