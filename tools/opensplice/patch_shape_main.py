#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Adds OPENSPLICE to the vendor chains of the OMG dds-rtps shape_main.cxx.

Each edit names the line it expects, so a changed upstream file fails here
instead of producing a binary that skips a QoS setting.
"""
import sys
p = sys.argv[1]
s = open(p).read()

def rep(old, new, count=1):
    global s
    n = s.count(old)
    if n != count:
        sys.exit(f"expected {count} x {old!r}, found {n}")
    s = s.replace(old, new)

rep('#elif defined(INTERCOM_DDS)\n#include "shape_configurator_intercom_dds.h"\n',
    '#elif defined(INTERCOM_DDS)\n#include "shape_configurator_intercom_dds.h"\n'
    '#elif defined(OPENSPLICE)\n#include "shape_configurator_opensplice.h"\n')
# Presentation (publisher and subscriber), begin/end_access: supported.
rep('#if   defined(RTI_CONNEXT_DDS) || defined(TWINOAKS_COREDX) || defined(INTERCOM_DDS)\n',
    '#if   defined(RTI_CONNEXT_DDS) || defined(TWINOAKS_COREDX) || defined(INTERCOM_DDS) || defined(OPENSPLICE)\n', 4)
# Representation: none in 6.9; refuse anything but XCDR1.
rep('#elif   defined(OPENDDS)\n        dw_qos.representation.value.length(1);\n        dw_qos.representation.value[0] = options->data_representation;\n',
    '#elif   defined(OPENDDS)\n        dw_qos.representation.value.length(1);\n        dw_qos.representation.value[0] = options->data_representation;\n\n'
    '#elif   defined(OPENSPLICE)\n        if (options->data_representation != XCDR_DATA_REPRESENTATION) {\n'
    '            logger.log_message("    Data_Representation = " + QosUtils::to_string(options->data_representation) + " : not supported", Verbosity::ERROR);\n'
    '            return false;\n        }\n')
rep('#elif   defined(OPENDDS)\n        dr_qos.representation.value.length(1);\n        dr_qos.representation.value[0] = options->data_representation;\n',
    '#elif   defined(OPENDDS)\n        dr_qos.representation.value.length(1);\n        dr_qos.representation.value[0] = options->data_representation;\n\n'
    '#elif   defined(OPENSPLICE)\n        if (options->data_representation != XCDR_DATA_REPRESENTATION) {\n'
    '            logger.log_message("    DataRepresentation = " + QosUtils::to_string(options->data_representation) + " : not supported", Verbosity::ERROR);\n'
    '            return false;\n        }\n')
rep('#if  defined(EPROSIMA_FAST_DDS)\n        logger.log_message("    Data_Representation = " + QosUtils::to_string(dw_qos.representation  FIELD_ACCESSOR.m_value[0]), Verbosity::DEBUG);\n#else',
    '#if  defined(EPROSIMA_FAST_DDS)\n        logger.log_message("    Data_Representation = " + QosUtils::to_string(dw_qos.representation  FIELD_ACCESSOR.m_value[0]), Verbosity::DEBUG);\n'
    '#elif defined(OPENSPLICE)\n        logger.log_message("    Data_Representation = " + QosUtils::to_string(options->data_representation), Verbosity::DEBUG);\n#else')
rep('#if defined(EPROSIMA_FAST_DDS)\n        logger.log_message("    DataRepresentation = " + QosUtils::to_string(dr_qos.representation().m_value[0]), Verbosity::DEBUG);\n#else',
    '#if defined(EPROSIMA_FAST_DDS)\n        logger.log_message("    DataRepresentation = " + QosUtils::to_string(dr_qos.representation().m_value[0]), Verbosity::DEBUG);\n'
    '#elif defined(OPENSPLICE)\n        logger.log_message("    DataRepresentation = " + QosUtils::to_string(options->data_representation), Verbosity::DEBUG);\n#else')
# Lifespan: supported.
rep('#elif defined(RTI_CONNEXT_DDS) || defined(OPENDDS) || defined(TWINOAKS_COREDX) || defined(INTERCOM_DDS)\n',
    '#elif defined(RTI_CONNEXT_DDS) || defined(OPENDDS) || defined(TWINOAKS_COREDX) || defined(INTERCOM_DDS) || defined(OPENSPLICE)\n')
# Content filter on color: the parameter unquoted, as for CoreDX and OpenDDS (measured: quoted, Test_Cft_0 received nothing).
rep('  #elif defined(TWINOAKS_COREDX) || defined(OPENDDS)\n',
    '  #elif defined(TWINOAKS_COREDX) || defined(OPENDDS) || defined(OPENSPLICE)\n')
# Sample sequences, sample access, previous handle, write, Duration_t aggregate.
rep('#if defined(RTI_CONNEXT_DDS) || defined(RTI_CONNEXT_MICRO) || defined(OPENDDS) || defined(INTERCOM_DDS)\n            ShapeTypeSeq          samples;',
    '#if defined(RTI_CONNEXT_DDS) || defined(RTI_CONNEXT_MICRO) || defined(OPENDDS) || defined(INTERCOM_DDS) || defined(OPENSPLICE)\n            ShapeTypeSeq          samples;')
rep('#if   defined(RTI_CONNEXT_DDS) || defined(RTI_CONNEXT_MICRO)\n                            ShapeType          *sample      = &samples[n_sample];',
    '#if   defined(RTI_CONNEXT_DDS) || defined(RTI_CONNEXT_MICRO) || defined(OPENSPLICE)\n                            ShapeType          *sample      = &samples[n_sample];')
rep('#if   defined(OPENDDS)\n                                if (sample->additional_payload_size.length() > 0) {',
    '#if   defined(OPENDDS) || defined(OPENSPLICE)\n                                if (sample->additional_payload_size.length() > 0) {')
rep('#if   defined(RTI_CONNEXT_DDS) || defined(RTI_CONNEXT_MICRO) ||  defined(OPENDDS) || defined(EPROSIMA_FAST_DDS) || defined(INTERCOM_DDS)\n',
    '#if   defined(RTI_CONNEXT_DDS) || defined(RTI_CONNEXT_MICRO) ||  defined(OPENDDS) || defined(EPROSIMA_FAST_DDS) || defined(INTERCOM_DDS) || defined(OPENSPLICE)\n')
rep('#if   defined(RTI_CONNEXT_DDS) || defined(RTI_CONNEXT_MICRO) || defined(OPENDDS) || defined(INTERCOM_DDS) || defined(TWINOAKS_COREDX)\n                    dws[i]->write( shape, HANDLE_NIL );',
    '#if   defined(RTI_CONNEXT_DDS) || defined(RTI_CONNEXT_MICRO) || defined(OPENDDS) || defined(INTERCOM_DDS) || defined(TWINOAKS_COREDX) || defined(OPENSPLICE)\n                    dws[i]->write( shape, HANDLE_NIL );')
rep('#if defined(RTI_CONNEXT_DDS) || defined (RTI_CONNEXT_MICRO) || defined (OPENDDS)\n        Duration_t max_wait = {1, 0};',
    '#if defined(RTI_CONNEXT_DDS) || defined (RTI_CONNEXT_MICRO) || defined (OPENDDS) || defined(OPENSPLICE)\n        Duration_t max_wait = {1, 0};')
# NOT_ALIVE samples: OpenSplice 6.9 get_key_value() answers an empty color for
# them (measured: NOT_ALIVE lines without a color, Cyclone and ZeroDDS writers alike),
# so the color is cached per instance as for OpenDDS.
rep('#elif defined(OPENDDS)\n        // get_key_value() does not reliably return the color for NOT_ALIVE samples',
    '#elif defined(OPENDDS) || defined(OPENSPLICE)\n        // get_key_value() does not reliably return the color for NOT_ALIVE samples')
rep('#elif defined(OPENDDS)\n                                {\n                                    const char* c = sample->color STRING_IN;',
    '#elif defined(OPENDDS) || defined(OPENSPLICE)\n                                {\n                                    const char* c = sample->color STRING_IN;')
rep('#elif defined(OPENDDS)\n                                {\n                                    auto it = instance_handle_color.find(sample_info->instance_handle);',
    '#elif defined(OPENDDS) || defined(OPENSPLICE)\n                                {\n                                    auto it = instance_handle_color.find(sample_info->instance_handle);')
open(p, 'w').write(s)
print("patched")
