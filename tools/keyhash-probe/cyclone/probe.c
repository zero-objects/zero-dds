/* cyclone-keyhash-probe.c
 *
 * Reproduces Cyclone DDS's serdata_default_get_keyhash() using the public
 * dds_cdrstream API of the installed libddsc, for the three types in
 * keyhash.idl. Every serialisation step is a vendor function; the only
 * thing restated here is the md5-vs-pad branch and the "nkeys == 0 means
 * an empty key buffer" rule from gen_serdata_key().
 *
 * References (Cyclone 11.0.1 source tree):
 *   src/core/ddsc/src/dds_serdata_default.c:789  serdata_default_get_keyhash
 *   src/core/ddsc/src/dds_serdata_default.c:275  gen_serdata_key
 *   src/core/cdr/src/dds_cdrstream.c:6948        dds_stream_key_flags
 */
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include "dds/dds.h"
#include "dds/cdr/dds_cdrstream.h"
#include "dds/ddsrt/md5.h"
#include "keyhash.h"

#ifndef DDS_FIXED_KEY_MAX_SIZE
#define DDS_FIXED_KEY_MAX_SIZE 16
#endif

static void hex (const char *label, const unsigned char *p, size_t n)
{
  printf ("%-46s", label);
  for (size_t i = 0; i < n; i++)
    printf ("%02x%s", p[i], (i + 1 < n) ? " " : "");
  printf ("\n");
}

static bool fixed_key_keyhash (uint32_t flagset, uint32_t xcdrv)
{
  /* is_topic_fixed_key_impl(..., key_hash = true),
     dds_serdata_default.c:252 */
  if (xcdrv == DDSI_RTPS_CDR_ENC_VERSION_1)
    return (flagset & DDS_TOPIC_FIXED_KEY) != 0;
  else
    return (flagset & DDS_TOPIC_FIXED_KEY_XCDR2_KEYHASH) != 0;
}

static void probe (const char *name, const dds_topic_descriptor_t *td,
                   const void *sample, uint32_t xcdrv)
{
  const struct dds_cdrstream_allocator *alloc = &dds_cdrstream_default_allocator;
  struct dds_cdrstream_desc desc;
  dds_cdrstream_desc_from_topic_desc (&desc, td);

  char tag[160];
  const char *v = (xcdrv == DDSI_RTPS_CDR_ENC_VERSION_1) ? "XCDR1" : "XCDR2";

  printf ("--- %s (%s) ---\n", name, v);
  printf ("  nkeys                                       %u\n", desc.keys.nkeys);
  printf ("  flagset                                     0x%08x  FIXED_KEY=%d FIXED_KEY_XCDR2=%d FIXED_KEY_XCDR2_KEYHASH=%d\n",
          desc.flagset,
          (desc.flagset & DDS_TOPIC_FIXED_KEY) ? 1 : 0,
          (desc.flagset & DDS_TOPIC_FIXED_KEY_XCDR2) ? 1 : 0,
          (desc.flagset & DDS_TOPIC_FIXED_KEY_XCDR2_KEYHASH) ? 1 : 0);

  /* 1. serialise the sample, as the writer does */
  dds_ostream_t os;
  dds_ostream_init (&os, alloc, 0, xcdrv);
  if (!dds_stream_write_sample (&os, alloc, sample, &desc)) {
    printf ("  dds_stream_write_sample FAILED\n");
    return;
  }
  snprintf (tag, sizeof tag, "  serialised sample (%s, LE, no CDR hdr)", v);
  hex (tag, os.m_buffer, os.m_index);

  /* 2. gen_serdata_key: the serdata's key is always XCDR2. nkeys == 0 gives
        an empty key buffer (dds_serdata_default.c:280-284). */
  unsigned char *keybuf = NULL;
  uint32_t keysize = 0;
  dds_ostream_t keyos;
  memset (&keyos, 0, sizeof keyos);
  if (desc.keys.nkeys > 0)
  {
    dds_istream_t is;
    dds_istream_init (&is, os.m_index, os.m_buffer, xcdrv);
    dds_ostream_init (&keyos, alloc, 0, DDSI_RTPS_CDR_ENC_VERSION_2);
    if (!dds_stream_extract_key_from_data (&is, &keyos, alloc, &desc)) {
      printf ("  dds_stream_extract_key_from_data FAILED\n");
      return;
    }
    keybuf = keyos.m_buffer;
    keysize = keyos.m_index;
  }
  /* A 64-byte zeroed scratch buffer stands in for the empty key buffer of a
     keyless serdata. The istream size stays 0, exactly as Cyclone would set
     it; the scratch only keeps the mutable-type reader, which reads a DHEADER
     before looking at the size, from running off the end of the allocation. */
  unsigned char scratch[64];
  memset (scratch, 0, sizeof scratch);
  hex ("  serdata key buffer (XCDR2)", keybuf ? keybuf : scratch, keysize);

  /* 3. serdata_default_get_keyhash */
  dds_istream_t kis;
  dds_istream_init (&kis, keysize, keybuf ? (const void *) keybuf : (const void *) scratch,
                    DDSI_RTPS_CDR_ENC_VERSION_2);
  dds_ostreamBE_t bos;
  dds_ostreamBE_init (&bos, alloc, 0, xcdrv);
  dds_stream_extract_keyBE_from_key (&kis, &bos, DDS_CDR_KEY_SERIALIZATION_KEYHASH, alloc, &desc);
  uint32_t actual_keysz = bos.x.m_index;
  hex ("  keyhash serialisation (BE, KEYHASH kind)", bos.x.m_buffer, actual_keysz);

  unsigned char kh[DDS_FIXED_KEY_MAX_SIZE];
  bool fixed = fixed_key_keyhash (desc.flagset, xcdrv);
  if (!fixed)
  {
    ddsrt_md5_state_t md5st;
    ddsrt_md5_init (&md5st);
    ddsrt_md5_append (&md5st, (ddsrt_md5_byte_t *) bos.x.m_buffer, actual_keysz);
    ddsrt_md5_finish (&md5st, (ddsrt_md5_byte_t *) kh);
  }
  else
  {
    memset (kh, 0, sizeof kh);
    if (actual_keysz > 0)
      memcpy (kh, bos.x.m_buffer, actual_keysz);
  }
  printf ("  path                                        %s\n",
          fixed ? "memset0 + memcpy (fixed key)" : "MD5");
  hex ("  KEYHASH", kh, 16);

  dds_ostreamBE_fini (&bos, alloc);
  if (desc.keys.nkeys > 0)
    dds_ostream_fini (&keyos, alloc);
  dds_ostream_fini (&os, alloc);
  dds_cdrstream_desc_fini (&desc, alloc);
  printf ("\n");
}

int main (void)
{
  Foo foo = { .id = 0x12345678, .x = 10, .y = 20 };
  FooApp fooapp = { .id = 0x12345678, .x = 10, .y = 20 };
  FooMut foomut = { .id = 0x12345678, .x = 10, .y = 20 };
  NoKeyFinal nkf = { .a = 0x12345678, .b = 20 };
  NoKeyMutable nkm = { .a = 0x12345678, .b = 20 };
  OuterKeylessNested okn = { .inner = { .a = 0x0a0a0a0a, .b = 0x0b0b0b0b }, .tail = 0x0c0c0c0c };
  OuterKeyedNested okd = { .inner = { .a = 0x0a0a0a0a, .b = 0x0b0b0b0b }, .tail = 0x0c0c0c0c };
static char *comb_names[2] = { "", "\xc3\xbc" "ber " "\xe1\x88\xb4" };
  CombSample comb;
  memset (&comb, 0, sizeof comb);
  comb.middle.leaf.k = -1234567;
  comb.middle.leaf.v = -2;
  comb.middle.names._length = 2;
  comb.middle.names._maximum = 2;
  comb.middle.names._release = false;
  comb.middle.names._buffer = comb_names;
  comb.tail = -1;
  CombSample comb0;
  memset (&comb0, 0, sizeof comb0);
  comb0.middle.leaf.k = -1234567;
  comb0.middle.leaf.v = -2;
  comb0.tail = -1;
  WholeOuter whole = { .inner = { .a = -1234567, .b = -2 }, .other = 1 };
  ChainOuter chain = { .mid = { .inner = { .p = 0x0d0d0d0d, .q = 0x0e0e0e0e }, .r = 0x0f0f0f0f }, .tail = 0x11111111 };
  DeepOuter deep = { .mid = { .leaf = { .p = 0x0d0d0d0d, .q = 0x0e0e0e0e }, .r = 0x0f0f0f0f }, .tail = 0x11111111 };

  PadKey padk = { .a = 0x11, .b = 0x22334455, .v = 1 };
  WideKey widek = { .s = 0x0102, .ll = 0x0304050607080910LL, .v = 1 };
  CharKey chark = { .o = 0x11, .s = 0x2233, .v = 1 };
  StrKey strk = { .name = (char *) "ab", .id = 0x0a0b0c0d, .v = 1 };
  PathKey pathk = { .nested = { .a = 0x55, .b = 0x66 }, .x = 0x77778888, .v = 1 };

  for (int i = 0; i < 2; i++)
  {
    uint32_t xcdrv = (i == 0) ? DDSI_RTPS_CDR_ENC_VERSION_1 : DDSI_RTPS_CDR_ENC_VERSION_2;
    probe ("@final    Foo{id=0x12345678,x=10,y=20}", &Foo_desc, &foo, xcdrv);
    if (xcdrv == DDSI_RTPS_CDR_ENC_VERSION_2)
    {
      probe ("@appendable FooApp{id=0x12345678,x=10,y=20}", &FooApp_desc, &fooapp, xcdrv);
      probe ("@mutable  FooMut{id=0x12345678,x=10,y=20}", &FooMut_desc, &foomut, xcdrv);
    }
    probe ("NoKeyFinal{a=0x12345678,b=20}", &NoKeyFinal_desc, &nkf, xcdrv);
    if (xcdrv == DDSI_RTPS_CDR_ENC_VERSION_2)
      probe ("NoKeyMutable{a=0x12345678,b=20}", &NoKeyMutable_desc, &nkm, xcdrv);
    else
      printf ("--- NoKeyMutable (XCDR1) --- skipped: @mutable requires XCDR2 in Cyclone\n\n");
    probe ("OuterKeylessNested{inner={0a..,0b..},tail=0c..}", &OuterKeylessNested_desc, &okn, xcdrv);
    probe ("OuterKeyedNested{inner={0a..,0b..},tail=0c..}", &OuterKeyedNested_desc, &okd, xcdrv);
    probe ("DeepOuter{mid={leaf={0d..,0e..},r=0f..},tail=11..}", &DeepOuter_desc, &deep, xcdrv);
    probe ("ChainOuter{mid={inner={0d..,0e..},r=0f..},tail=11..}", &ChainOuter_desc, &chain, xcdrv);
    probe ("WholeOuter{inner={-1234567,-2},other=1}", &WholeOuter_desc, &whole, xcdrv);
    if (xcdrv == DDSI_RTPS_CDR_ENC_VERSION_2)
    {
      probe ("CombSample (combined-everything key path)", &CombSample_desc, &comb, xcdrv);
      probe ("CombSample names=[]", &CombSample_desc, &comb0, xcdrv);
    }
    probe ("PadKey{a=0x11,b=0x22334455}", &PadKey_desc, &padk, xcdrv);
    probe ("WideKey{s=0x0102,ll=0x0304050607080910}", &WideKey_desc, &widek, xcdrv);
    probe ("CharKey{o=0x11,s=0x2233}", &CharKey_desc, &chark, xcdrv);
    probe ("StrKey{name=ab,id=0x0a0b0c0d}", &StrKey_desc, &strk, xcdrv);
    probe ("PathKey{nested={0x55,0x66},x=0x77778888}", &PathKey_desc, &pathk, xcdrv);
  }
  return 0;
}
