/* rti-keyhash-probe.c
 *
 * Drives RTI Connext's type plugin directly, with no DomainParticipant and
 * therefore no licence check, in the same way tools/wireprobe/rti-template.c
 * does for the payload. For each type it prints
 *   - the plugin's key kind,
 *   - the serialised FooKeyHolder bytes produced by
 *     PRESTypePlugin_interpretedSerializeKeyForKeyhash,
 *   - the 16-byte keyhash produced by PRESTypePlugin_interpretedInstanceToKeyHash.
 *
 * The keyless types have instanceToKeyHashFnc == NULL in the generated
 * plugin (keyhashPlugin.c), so for those the interpreted function is called
 * directly to show what the interpreter would do if it were reachable.
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

static char KEYBUF[65536];
static char MD5BUF[65536];

static void hex (const char *label, const unsigned char *p, unsigned int n)
{
  printf ("  %-44s", label);
  if (n == 0) printf ("(empty)");
  for (unsigned int i = 0; i < n; i++)
    printf ("%02x%s", p[i], (i + 1 < n) ? " " : "");
  printf ("\n");
}

#define PROBE(T, KEYKIND_EXPECTED, sample_setup)                                    \
  do {                                                                              \
    struct RTIXCdrTypePluginProgramContext program_context =                        \
        RTIXCdrTypePluginProgramContext_INTIALIZER;                                 \
    struct PRESTypePluginDefaultParticipantData participant_data;                   \
    struct PRESTypePluginDefaultEndpointData endpoint_data;                         \
    struct PRESTypePlugin plugin;                                                   \
    struct RTICdrStream keystream;                                                  \
    T sample;                                                                       \
                                                                                    \
    printf ("--- %s ---\n", #T);                                                    \
    if (!T##_initialize (&sample)) { printf ("  initialize failed\n"); break; }      \
    sample_setup;                                                                   \
                                                                                    \
    RTIOsapiMemory_zero (&participant_data, sizeof (participant_data));             \
    RTIOsapiMemory_zero (&endpoint_data, sizeof (endpoint_data));                   \
    RTIOsapiMemory_zero (&plugin, sizeof (plugin));                                 \
    endpoint_data.programContext = program_context;                                 \
    endpoint_data._participantData = &participant_data;                             \
    endpoint_data.typePlugin = &plugin;                                             \
    endpoint_data.programContext.endpointPluginData = &endpoint_data;               \
    plugin.typeCode = (struct RTICdrTypeCode *) T##_get_typecode ();                \
    participant_data.programs = T##Plugin_get_programs ();                          \
    if (participant_data.programs == NULL) {                                        \
      printf ("  no interpreter programs\n"); break; }                              \
                                                                                    \
    printf ("  get_key_kind()                               %d (%s)\n",             \
            (int) T##Plugin_get_key_kind (),                                        \
            T##Plugin_get_key_kind () == PRES_TYPEPLUGIN_NO_KEY ? "PRES_TYPEPLUGIN_NO_KEY" \
            : (T##Plugin_get_key_kind () == PRES_TYPEPLUGIN_USER_KEY                \
               ? "PRES_TYPEPLUGIN_USER_KEY" : "other"));                            \
                                                                                    \
    endpoint_data._maxSizeSerializedKeyV1 =                                         \
      PRESTypePlugin_interpretedGetSerializedKeyMaxSize (                           \
        (PRESTypePluginEndpointData) &endpoint_data, NULL, RTI_FALSE,               \
        RTI_CDR_ENCAPSULATION_ID_CDR_BE, 0);                                        \
    endpoint_data._maxSizeSerializedKeyV2 =                                         \
      PRESTypePlugin_interpretedGetSerializedKeyMaxSizeForKeyhash (                 \
        (PRESTypePluginEndpointData) &endpoint_data, NULL,                          \
        RTI_CDR_ENCAPSULATION_ID_CDR2_BE, 0);                                       \
    printf ("  serialized_key_max_size (CDR_BE)             %u\n",                  \
            endpoint_data._maxSizeSerializedKeyV1);                                 \
    printf ("  serialized_key_max_size_for_keyhash (CDR2_BE) %u\n",                 \
            endpoint_data._maxSizeSerializedKeyV2);                                 \
                                                                                    \
    RTICdrStream_init (&endpoint_data._md5Stream);                                  \
    RTICdrStream_set (&endpoint_data._md5Stream, MD5BUF, (int) sizeof (MD5BUF));    \
                                                                                    \
    RTICdrStream_init (&keystream);                                                 \
    RTICdrStream_set (&keystream, KEYBUF, (int) sizeof (KEYBUF));                   \
    memset (KEYBUF, 0, 64);                                                         \
    if (PRESTypePlugin_interpretedSerializeKeyForKeyhash (                          \
          (PRESTypePluginEndpointData) &endpoint_data, &sample, &keystream,         \
          RTI_CDR_ENCAPSULATION_ID_CDR2_BE, NULL)) {                                \
      unsigned int n = (unsigned int) RTICdrStream_getCurrentPositionOffset (&keystream); \
      hex ("serializeKeyForKeyhash (CDR2_BE)", (unsigned char *) KEYBUF, n);        \
    } else {                                                                        \
      printf ("  serializeKeyForKeyhash                       returned FALSE\n");   \
    }                                                                               \
                                                                                    \
    { struct MIGRtpsKeyHash kh;                                                     \
      memset (&kh, 0xAA, sizeof kh);                                                \
      kh.length = 16;                                                               \
      if (PRESTypePlugin_interpretedInstanceToKeyHash (                             \
            (PRESTypePluginEndpointData) &endpoint_data, &kh, &sample,              \
            RTI_CDR_ENCAPSULATION_ID_CDR2_BE)) {                                    \
        printf ("  interpretedInstanceToKeyHash                 returned TRUE, length=%u\n", \
                (unsigned) kh.length);                                              \
        hex ("KEYHASH", kh.value, 16);                                              \
      } else {                                                                      \
        printf ("  interpretedInstanceToKeyHash                 returned FALSE\n"); \
        hex ("KEYHASH (untouched, prefilled 0xAA)", kh.value, 16);                  \
      }                                                                             \
    }                                                                               \
    printf ("\n");                                                                  \
  } while (0)

int main (void)
{
  PROBE (Foo, PRES_TYPEPLUGIN_USER_KEY,
         do { sample.id = 0x12345678; sample.x = 10; sample.y = 20; } while (0));
  PROBE (NoKeyFinal, PRES_TYPEPLUGIN_NO_KEY,
         do { sample.a = 0x12345678; sample.b = 20; } while (0));
  PROBE (NoKeyMutable, PRES_TYPEPLUGIN_NO_KEY,
         do { sample.a = 0x12345678; sample.b = 20; } while (0));
  return 0;
}
