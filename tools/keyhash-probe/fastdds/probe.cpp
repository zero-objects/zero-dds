/* fastdds-keyhash-probe.cpp
 *
 * Calls the fastddsgen-generated <T>PubSubType::compute_key for the three
 * types in keyhash.idl and prints the InstanceHandle_t bytes.
 *
 * The handle is pre-filled with 0xAA so that "compute_key left it alone"
 * is distinguishable from "compute_key wrote sixteen zeros".
 */
#include <cstdio>
#include <cstring>
#include <exception>
#include <string>
#include <vector>
#include <sys/wait.h>
#include <unistd.h>
#include "keyhashPubSubTypes.hpp"

using eprosima::fastdds::rtps::InstanceHandle_t;

static void show(const char* label, bool ok, const InstanceHandle_t& h)
{
    printf("  %-34s ret=%-5s handle=", label, ok ? "true" : "false");
    for (int i = 0; i < 16; ++i) printf("%02x%s", h.value[i], i < 15 ? " " : "");
    printf("\n");
}

template <typename PST, typename T>
static void probe(const char* name, const T& sample)
{
    PST pst;
    printf("--- %s ---\n", name);
    printf("  is_compute_key_provided            %s\n",
           pst.is_compute_key_provided ? "true" : "false");
    printf("  max_serialized_type_size           %u\n", pst.max_serialized_type_size);

    InstanceHandle_t h;
    try {
        memset(h.value, 0xAA, 16);
        bool ok = pst.compute_key(static_cast<const void*>(&sample), h, false);
        show("compute_key(data, h, force_md5=false)", ok, h);

        memset(h.value, 0xAA, 16);
        ok = pst.compute_key(static_cast<const void*>(&sample), h, true);
        show("compute_key(data, h, force_md5=true) ", ok, h);
    } catch (const std::exception& e) {
        printf("  compute_key threw %s\n", e.what());
    }
    printf("\n");
}

/* compute_key of a type whose key holds an unbounded member serializes into
 * a buffer sized by fastddsgen's max_key_cdr_typesize, which leaves that
 * member out. A longer key throws NotEnoughMemoryException out of a
 * noexcept frame, and the process aborts; in a child, the abort is the
 * measurement and the probes after it still run. */
template <typename PST, typename T>
static void probe_in_child(const char* name, const T& sample)
{
    pid_t pid = fork();
    if (pid == 0)
    {
        probe<PST>(name, sample);
        _exit(0);
    }
    int status = 0;
    waitpid(pid, &status, 0);
    if (WIFSIGNALED(status))
    {
        printf("  the process ended on signal %d\n\n", WTERMSIG(status));
    }
}

int main()
{
    setvbuf(stdout, nullptr, _IONBF, 0);
    Foo foo;
    foo.id(0x12345678);
    foo.x(10);
    foo.y(20);

    NoKeyFinal nkf;
    nkf.a(0x12345678);
    nkf.b(20);

    NoKeyMutable nkm;
    nkm.a(0x12345678);
    nkm.b(20);

    FooApp fooapp;
    fooapp.id(0x12345678); fooapp.x(10); fooapp.y(20);
    FooMut foomut;
    foomut.id(0x12345678); foomut.x(10); foomut.y(20);

    probe<FooPubSubType>("@final Foo{id=0x12345678,x=10,y=20}", foo);
    probe<FooAppPubSubType>("@appendable FooApp{id=0x12345678,x=10,y=20}", fooapp);
    probe<FooMutPubSubType>("@mutable FooMut{id=0x12345678,x=10,y=20}", foomut);
    probe<NoKeyFinalPubSubType>("NoKeyFinal{a=0x12345678,b=20}", nkf);
    probe<NoKeyMutablePubSubType>("NoKeyMutable{a=0x12345678,b=20}", nkm);
    NestedNoKey nnk; nnk.a(0x0a0a0a0a); nnk.b(0x0b0b0b0b);
    OuterKeylessNested okn; okn.inner(nnk); okn.tail(0x0c0c0c0c);

    NestedSomeKey nsk; nsk.a(0x0a0a0a0a); nsk.b(0x0b0b0b0b);
    OuterKeyedNested okd; okd.inner(nsk); okd.tail(0x0c0c0c0c);

    DeepLeaf dl; dl.p(0x0d0d0d0d); dl.q(0x0e0e0e0e);
    DeepMid dm; dm.leaf(dl); dm.r(0x0f0f0f0f);
    DeepOuter deep; deep.mid(dm); deep.tail(0x11111111);

    probe<OuterKeylessNestedPubSubType>("OuterKeylessNested{inner={0a..,0b..},tail=0c..}", okn);
    probe<OuterKeyedNestedPubSubType>("OuterKeyedNested{inner={0a..,0b..},tail=0c..}", okd);
    probe<DeepOuterPubSubType>("DeepOuter{mid={leaf={0d..,0e..},r=0f..},tail=11..}", deep);
    ChainLeaf cl; cl.p(0x0d0d0d0d); cl.q(0x0e0e0e0e);
    ChainMid cm; cm.inner(cl); cm.r(0x0f0f0f0f);
    ChainOuter chain; chain.mid(cm); chain.tail(0x11111111);
    probe<ChainOuterPubSubType>("ChainOuter{mid={inner={0d..,0e..},r=0f..},tail=11..}", chain);
    CombLeaf cbl; cbl.k(-1234567); cbl.v(-2);
    CombMiddle cbm; cbm.leaf(cbl);
    cbm.names({std::string(""), std::string("\xc3\xbc" "ber " "\xe1\x88\xb4")});
    CombSample cbs; cbs.middle(cbm); cbs.tail(-1);
    WholeInner wi; wi.a(-1234567); wi.b(-2);
    WholeOuter wo; wo.inner(wi); wo.other(1);
    probe<WholeOuterPubSubType>("WholeOuter{inner={-1234567,-2},other=1}", wo);
    PadKey padk; padk.a(0x11); padk.b(0x22334455); padk.v(1);
    probe<PadKeyPubSubType>("PadKey{a=0x11,b=0x22334455}", padk);
    WideKey widek; widek.s(0x0102); widek.ll(0x0304050607080910LL); widek.v(1);
    probe<WideKeyPubSubType>("WideKey{s=0x0102,ll=0x0304050607080910}", widek);
    CharKey chark; chark.o(0x11); chark.s(0x2233); chark.v(1);
    probe<CharKeyPubSubType>("CharKey{o=0x11,s=0x2233}", chark);
    StrKey strk; strk.name("ab"); strk.id(0x0a0b0c0d); strk.v(1);
    probe<StrKeyPubSubType>("StrKey{name=ab,id=0x0a0b0c0d}", strk);
    PathLeaf pathl; pathl.a(0x55); pathl.b(0x66);
    PathKey pathk; pathk.nested(pathl); pathk.x(0x77778888); pathk.v(1);
    probe<PathKeyPubSubType>("PathKey{nested={0x55,0x66},x=0x77778888}", pathk);
    probe_in_child<CombSamplePubSubType>("CombSample (combined-everything key path)", cbs);
    // The same shape with no names: the holder fits the 20 octets.
    CombMiddle cbm0; cbm0.leaf(cbl);
    CombSample cbs0; cbs0.middle(cbm0); cbs0.tail(-1);
    probe_in_child<CombSamplePubSubType>("CombSample names=[]", cbs0);
    return 0;
}
