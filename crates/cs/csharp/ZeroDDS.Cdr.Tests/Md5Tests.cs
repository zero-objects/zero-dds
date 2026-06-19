// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// MD5 implementation tests.  Vector source: RFC 1321 appendix A.5 +
// NIST FIPS-180-Test-Suite.

using System;
using System.Text;
using Xunit;
using ZeroDDS.Cdr;

namespace ZeroDDS.Cdr.Tests;

public class Md5Tests
{
    private static string ToHex(byte[] bytes) => BitConverter.ToString(bytes).Replace("-", "").ToLowerInvariant();

    [Fact]
    public void EmptyInput_ReturnsRfc1321ExpectedHash()
    {
        var hash = Md5.Hash(Array.Empty<byte>());
        Assert.Equal("d41d8cd98f00b204e9800998ecf8427e", ToHex(hash));
    }

    [Fact]
    public void SingleA_ReturnsExpectedHash()
    {
        var hash = Md5.Hash(Encoding.ASCII.GetBytes("a"));
        Assert.Equal("0cc175b9c0f1b6a831c399e269772661", ToHex(hash));
    }

    [Fact]
    public void Abc_ReturnsExpectedHash()
    {
        var hash = Md5.Hash(Encoding.ASCII.GetBytes("abc"));
        Assert.Equal("900150983cd24fb0d6963f7d28e17f72", ToHex(hash));
    }

    [Fact]
    public void MessageDigest_ReturnsExpectedHash()
    {
        var hash = Md5.Hash(Encoding.ASCII.GetBytes("message digest"));
        Assert.Equal("f96b697d7cb7938d525a2f31aaf161d0", ToHex(hash));
    }

    [Fact]
    public void Alphabet_ReturnsExpectedHash()
    {
        var hash = Md5.Hash(Encoding.ASCII.GetBytes("abcdefghijklmnopqrstuvwxyz"));
        Assert.Equal("c3fcd3d76192e4007dfb496cca67e13b", ToHex(hash));
    }

    [Fact]
    public void V8KeyHolder_BytesProduceCorrectMd5()
    {
        // PlainCdr2BeKeyHolder for V-8 (4-byte BE id=42).
        var hash = Md5.Hash(new byte[] { 0x00, 0x00, 0x00, 0x2A });
        // Hash matches `md5sum`: a515855799ddbda08bc99fc2ce87fa79.
        Assert.Equal("a515855799ddbda08bc99fc2ce87fa79", ToHex(hash));
    }
}
