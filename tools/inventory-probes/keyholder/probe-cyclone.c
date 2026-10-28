#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include "dds/dds.h"
#include "dds/cdr/dds_cdrstream.h"
#include "dds/ddsrt/md5.h"
#include "kh.h"

static void hex(const char *l, const unsigned char *p, size_t n) {
  printf("%-34s", l);
  for (size_t i = 0; i < n; i++) printf("%02x", p[i]);
  printf("  (%zu)\n", n);
}

static void probe(const char *name, const dds_topic_descriptor_t *td,
                  const void *sample, uint32_t xcdrv) {
  const struct dds_cdrstream_allocator *alloc = &dds_cdrstream_default_allocator;
  struct dds_cdrstream_desc desc;
  dds_cdrstream_desc_from_topic_desc(&desc, td);
  printf("--- %s (XCDR%u) nkeys=%u ---\n", name, xcdrv, desc.keys.nkeys);
  dds_ostream_t os; dds_ostream_init(&os, alloc, 0, xcdrv);
  if (!dds_stream_write_sample(&os, alloc, sample, &desc)) { printf("  write FAILED\n"); return; }
  hex("  sample", os.m_buffer, os.m_index);
  dds_istream_t is; dds_istream_init(&is, os.m_index, os.m_buffer, xcdrv);
  dds_ostream_t keyos; dds_ostream_init(&keyos, alloc, 0, DDSI_RTPS_CDR_ENC_VERSION_2);
  if (!dds_stream_extract_key_from_data(&is, &keyos, alloc, &desc)) { printf("  extract FAILED\n"); return; }
  hex("  key holder (XCDR2 LE)", keyos.m_buffer, keyos.m_index);
  dds_istream_t kis; dds_istream_init(&kis, keyos.m_index, keyos.m_buffer, DDSI_RTPS_CDR_ENC_VERSION_2);
  dds_ostreamBE_t bos; dds_ostreamBE_init(&bos, alloc, 0, xcdrv);
  dds_stream_extract_keyBE_from_key(&kis, &bos, DDS_CDR_KEY_SERIALIZATION_KEYHASH, alloc, &desc);
  hex("  keyhash serialisation (BE)", bos.x.m_buffer, bos.x.m_index);
  unsigned char kh[16]; memset(kh, 0, 16);
  int fixed = (xcdrv == DDSI_RTPS_CDR_ENC_VERSION_1)
      ? ((desc.flagset & DDS_TOPIC_FIXED_KEY) != 0)
      : ((desc.flagset & DDS_TOPIC_FIXED_KEY_XCDR2_KEYHASH) != 0);
  if (fixed) { if (bos.x.m_index) memcpy(kh, bos.x.m_buffer, bos.x.m_index); }
  else { ddsrt_md5_state_t s; ddsrt_md5_init(&s);
         ddsrt_md5_append(&s, (ddsrt_md5_byte_t *)bos.x.m_buffer, bos.x.m_index);
         ddsrt_md5_finish(&s, (ddsrt_md5_byte_t *)kh); }
  printf("  path                              %s\n", fixed ? "fixed (zero-extended)" : "MD5");
  hex("  KEYHASH", kh, 16);
  printf("\n");
}

int main(void) {
  KeyOrder ko = { .b = 2, .a = 1 };
  KeyMixed km = { .b = 2, .a = 1 };
  KeyBig kb = { .a = 1, .b = 2, .c = 3, .d = 4, .e = 5 };
  for (int i = 0; i < 2; i++) {
    uint32_t v = i ? DDSI_RTPS_CDR_ENC_VERSION_2 : DDSI_RTPS_CDR_ENC_VERSION_1;
    probe("KeyOrder{@id(5)@key b=2, @id(2)@key a=1}", &KeyOrder_desc, &ko, v);
    probe("KeyBig{a..e = 1..5}", &KeyBig_desc, &kb, v);
    probe("KeyMixed{@id(5)@key octet b=2, @id(2)@key long a=1}", &KeyMixed_desc, &km, v);
  }
  return 0;
}
