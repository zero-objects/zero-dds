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

#include "kh.h"
#include "khSupport.h"
#include "khPlugin.h"

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

  PROBE (KeyOrder, do { sample.b = 2; sample.a = 1; } while (0));
  PROBE (KeyBig, do { sample.a = 1; sample.b = 2; sample.c = 3; sample.d = 4; sample.e = 5; } while (0));
  PROBE (KeyMixed, do { sample.b = 2; sample.a = 1; } while (0));
  return 0;
}
