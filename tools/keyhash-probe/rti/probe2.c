/* rti-keyhash-probe2.c
 *
 * Same intent as probe.c, but it builds the endpoint data through the
 * generated <T>Plugin_on_participant_attached / _on_endpoint_attached
 * entry points, which is what asserts the interpreter's keyhash programs
 * (DDS_TypeCodeFactory_assert_programs_in_global_list with
 * RTI_XCDR_PROGRAM_MASK_TYPEPLUGIN). No DomainParticipant is created, so
 * no licence is needed.
 */
#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#include "ndds/ndds_c.h"
#include "dds_c/dds_c_typecode_impl.h"
#include "cdr/cdr_encapsulation.h"
#include "cdr/cdr_stream.h"
#include "pres/pres_typePlugin.h"
#include "xcdr/xcdr_interpreter.h"
#include "mig/mig_rtps.h"

#include "keyhash.h"
#include "keyhashSupport.h"
#include "keyhashPlugin.h"

static void hex (const char *label, const unsigned char *p, unsigned int n)
{
  printf ("  %-44s", label);
  if (n == 0) printf ("(empty)");
  for (unsigned int i = 0; i < n; i++)
    printf ("%02x%s", p[i], (i + 1 < n) ? " " : "");
  printf ("\n");
}

static struct RTIEncapsulationInfo ENCAPS[8];

static const RTIEncapsulationId ENCLIST[] = {
  RTI_CDR_ENCAPSULATION_ID_CDR_BE,
  RTI_CDR_ENCAPSULATION_ID_PL_CDR_BE,
  RTI_CDR_ENCAPSULATION_ID_CDR2_BE,
  RTI_CDR_ENCAPSULATION_ID_D_CDR2_BE,
  RTI_CDR_ENCAPSULATION_ID_PL_CDR2_BE
};
static const char *const ENCNAMES[] = {
  "CDR_BE   (XCDR1 plain)",
  "PL_CDR_BE(XCDR1 param-list)",
  "CDR2_BE  (XCDR2 plain)",
  "D_CDR2_BE(XCDR2 delimited)",
  "PL_CDR2_BE(XCDR2 param-list)"
};

#define PROBE(T, sample_setup)                                                        \
  do {                                                                                \
    struct PRESTypePluginParticipantInfo pinfo;                                       \
    struct PRESTypePluginEndpointInfo einfo;                                          \
    PRESTypePluginParticipantData pd;                                                 \
    PRESTypePluginEndpointData epd;                                                   \
    struct PRESTypePlugin *plugin;                                                    \
    T sample;                                                                         \
                                                                                      \
    printf ("--- %s ---\n", #T);                                                      \
    plugin = T##Plugin_new ();                                                        \
    if (plugin == NULL) { printf ("  Plugin_new failed\n"); break; }                  \
    printf ("  plugin->instanceToKeyHashFnc                 %s\n",                    \
            plugin->instanceToKeyHashFnc ? "set" : "NULL");                           \
    printf ("  plugin->serializedSampleToKeyHashFnc         %s\n",                    \
            plugin->serializedSampleToKeyHashFnc ? "set" : "NULL");                   \
    printf ("  plugin->getKeyKindFnc()                      %d (%s)\n",               \
            (int) plugin->getKeyKindFnc (),                                           \
            plugin->getKeyKindFnc () == PRES_TYPEPLUGIN_NO_KEY                        \
              ? "PRES_TYPEPLUGIN_NO_KEY" : "PRES_TYPEPLUGIN_USER_KEY");               \
                                                                                      \
    if (!T##_initialize (&sample)) { printf ("  initialize failed\n"); break; }        \
    sample_setup;                                                                     \
                                                                                      \
    memset (&pinfo, 0, sizeof pinfo);                                                 \
    pd = plugin->onParticipantAttached (NULL, &pinfo, RTI_TRUE, NULL,                 \
                                        (RTICdrTypeCode *) plugin->typeCode);         \
    if (pd == NULL) { printf ("  onParticipantAttached failed\n"); break; }           \
                                                                                      \
    memset (&einfo, 0, sizeof einfo);                                                 \
    einfo.endpointKind = PRES_TYPEPLUGIN_ENDPOINT_WRITER;                             \
    einfo.isKeyed = (plugin->getKeyKindFnc () != PRES_TYPEPLUGIN_NO_KEY);             \
    einfo.initialSamples = 1;                                                         \
    einfo.maxSamples = 1;                                                             \
    einfo.initialInstances = 1;                                                       \
    einfo.maxInstances = 1;                                                           \
    einfo.requireMD5KeyHash = RTI_FALSE;                                              \
    einfo.typePlugin = plugin;                                                        \
    einfo.encapsulationCount = 8;                                                     \
    einfo.encapsulations = ENCAPS;                                                    \
    epd = plugin->onEndpointAttached (pd, &einfo, RTI_TRUE, NULL);                    \
    if (epd == NULL) { printf ("  onEndpointAttached failed\n"); break; }             \
                                                                                      \
    printf ("  maxSizeSerializedKey V1 / V2                 %u / %u\n",               \
            PRESTypePluginDefaultEndpointData_getMaxSizeSerializedKey (epd, 0),       \
            PRESTypePluginDefaultEndpointData_getMaxSizeSerializedKey (epd, 1));      \
                                                                                      \
    for (int e = 0; e < (int) (sizeof (ENCLIST) / sizeof (ENCLIST[0])); e++) {        \
      RTIEncapsulationId enc = ENCLIST[e];                                            \
      const char *encname = ENCNAMES[e];                                              \
      char keybuf[4096];                                                              \
      struct RTICdrStream keystream;                                                  \
      char lbl[96];                                                                   \
      memset (keybuf, 0, sizeof keybuf);                                              \
      RTICdrStream_init (&keystream);                                                 \
      RTICdrStream_set (&keystream, keybuf, (int) sizeof (keybuf));                   \
      snprintf (lbl, sizeof lbl, "serializeKeyForKeyhash %s", encname);               \
      if (PRESTypePlugin_interpretedSerializeKeyForKeyhash (                          \
            epd, &sample, &keystream, enc, NULL)) {                                   \
        hex (lbl, (unsigned char *) keybuf,                                           \
             (unsigned int) RTICdrStream_getCurrentPositionOffset (&keystream));      \
      } else {                                                                        \
        printf ("  %-44s FALSE\n", lbl);                                              \
      }                                                                               \
      struct MIGRtpsKeyHash kh;                                                       \
      memset (&kh, 0xAA, sizeof kh);                                                  \
      kh.length = 16;                                                                 \
      snprintf (lbl, sizeof lbl, "KEYHASH %s", encname);                              \
      if (plugin->instanceToKeyHashFnc == NULL) {                                     \
        printf ("  %-44s no instanceToKeyHashFnc in this plugin\n", lbl);             \
      } else if (plugin->instanceToKeyHashFnc (epd, &kh, &sample, enc)) {             \
        hex (lbl, kh.value, 16);                                                      \
      } else {                                                                        \
        printf ("  %-44s FALSE\n", lbl);                                              \
      }                                                                               \
    }                                                                                 \
    printf ("\n");                                                                    \
  } while (0)

int main (void)
{
  memset (ENCAPS, 0, sizeof ENCAPS);
  ENCAPS[0].encapsulationId = RTI_CDR_ENCAPSULATION_ID_CDR_BE;
  ENCAPS[1].encapsulationId = RTI_CDR_ENCAPSULATION_ID_CDR_LE;
  ENCAPS[2].encapsulationId = RTI_CDR_ENCAPSULATION_ID_PL_CDR_BE;
  ENCAPS[3].encapsulationId = RTI_CDR_ENCAPSULATION_ID_PL_CDR_LE;
  ENCAPS[4].encapsulationId = RTI_CDR_ENCAPSULATION_ID_CDR2_BE;
  ENCAPS[5].encapsulationId = RTI_CDR_ENCAPSULATION_ID_CDR2_LE;
  ENCAPS[6].encapsulationId = RTI_CDR_ENCAPSULATION_ID_D_CDR2_BE;
  ENCAPS[7].encapsulationId = RTI_CDR_ENCAPSULATION_ID_PL_CDR2_BE;

  PROBE (Foo, do { sample.id = 0x12345678; sample.x = 10; sample.y = 20; } while (0));
  PROBE (NoKeyFinal, do { sample.a = 0x12345678; sample.b = 20; } while (0));
  PROBE (NoKeyMutable, do { sample.a = 0x12345678; sample.b = 20; } while (0));
  PROBE (FooApp, do { sample.id = 0x12345678; sample.x = 10; sample.y = 20; } while (0));
  PROBE (FooMut, do { sample.id = 0x12345678; sample.x = 10; sample.y = 20; } while (0));
  PROBE (OuterKeylessNested, do { sample.inner.a = 0x0a0a0a0a; sample.inner.b = 0x0b0b0b0b;
                                  sample.tail = 0x0c0c0c0c; } while (0));
  PROBE (OuterKeyedNested, do { sample.inner.a = 0x0a0a0a0a; sample.inner.b = 0x0b0b0b0b;
                                sample.tail = 0x0c0c0c0c; } while (0));
  PROBE (CombSample, do { sample.middle.leaf.k = -1234567; sample.middle.leaf.v = -2;
                          DDS_StringSeq_set_maximum (&sample.middle.names, 2);
                          DDS_StringSeq_set_length (&sample.middle.names, 2);
                          *DDS_StringSeq_get_reference (&sample.middle.names, 0) =
                              DDS_String_dup ("");
                          *DDS_StringSeq_get_reference (&sample.middle.names, 1) =
                              DDS_String_dup ("\xc3\xbc" "ber " "\xe1\x88\xb4");
                          sample.tail = -1; } while (0));
  printf ("(the same, names = [])\n");
  PROBE (CombSample, do { sample.middle.leaf.k = -1234567; sample.middle.leaf.v = -2;
                          sample.tail = -1; } while (0));
  PROBE (WholeOuter, do { sample.inner.a = -1234567; sample.inner.b = -2;
                          sample.other = 1; } while (0));
  PROBE (ChainOuter, do { sample.mid.inner.p = 0x0d0d0d0d; sample.mid.inner.q = 0x0e0e0e0e;
                          sample.mid.r = 0x0f0f0f0f; sample.tail = 0x11111111; } while (0));
  PROBE (DeepOuter, do { sample.mid.leaf.p = 0x0d0d0d0d; sample.mid.leaf.q = 0x0e0e0e0e;
                         sample.mid.r = 0x0f0f0f0f; sample.tail = 0x11111111; } while (0));
  PROBE (PadKey, do { sample.a = 0x11; sample.b = 0x22334455; sample.v = 1; } while (0));
  PROBE (WideKey, do { sample.s = 0x0102; sample.ll = 0x0304050607080910LL; sample.v = 1; } while (0));
  PROBE (CharKey, do { sample.o = 0x11; sample.s = 0x2233; sample.v = 1; } while (0));
  PROBE (StrKey, do { sample.name = DDS_String_dup ("ab"); sample.id = 0x0a0b0c0d;
                      sample.v = 1; } while (0));
  PROBE (PathKey, do { sample.nested.a = 0x55; sample.nested.b = 0x66; sample.x = 0x77778888;
                       sample.v = 1; } while (0));
  return 0;
}
