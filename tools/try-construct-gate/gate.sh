#!/bin/bash
# try_construct gate: a writer's type serialises, a narrower reader's
# type deserialises, and what the reader does is the measurement.
#
#   gate.sh build-writer <case> <vendor>
#   gate.sh build-reader <case> <vendor>
#   gate.sh write <case> <vendor>                  -> "xcdr1 <hex>" lines
#   gate.sh read  <case> <vendor> <enc> <hex>      -> the reader's answer
#   gate.sh all   <case> [enc]                     -> the whole row
#
# A case directory holds writer.idl, reader.idl, type, and the fill/dump
# fragments each language column needs.
#
# It runs from the checkout: `ROOT` is the directory this file is in,
# not a copy of it deployed to the test host. Where each vendor lives is
# `tools/interop-vendors.sh`, so one setting per vendor moves the gate to
# a machine that puts them elsewhere — the pinned `ci-interop` image
# does.
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../interop-vendors.sh
. "$(cd "$ROOT/../.." && pwd)/tools/interop-vendors.sh"
CYCLONE="$CYCLONE_HOME"
RTILIB="$RTI_LIB"
export LD_LIBRARY_PATH="$OPENDDS_HOME/lib:${LD_LIBRARY_PATH:-}"

# `side` is writer or reader; it picks the IDL and the template.
build() {
    local case_name=$1 vendor=$2 side=$3 enc=${4:-xcdr2}
    local dir=$ROOT/cases/$case_name
    local work=$ROOT/work/$case_name/$vendor-$side-$enc
    local type; type=$(cat "$dir/type")
    local idl; idl=$dir/$side.idl
    local frag; frag=$( [ "$side" = writer ] && echo fill || echo dump )
    rm -rf "$work"; mkdir -p "$work"

    case $vendor in
      cyclone)
        cp "$idl" "$work/gen.idl"
        cp "$dir/$frag.c.inc" "$work/$frag.inc" 2>/dev/null || { echo "no $frag.c.inc"; return 1; }
        local version=DDSI_RTPS_CDR_ENC_VERSION_2
        [ "$enc" = xcdr1 ] && version=DDSI_RTPS_CDR_ENC_VERSION_1
        sed -e "s/@TYPE@/$type/g" -e "s/@VERSION@/$version/g" \
            "$ROOT/tpl/cyclone-$( [ "$side" = writer ] && echo w.c || echo r.c )" > "$work/ref.c"
        ( cd "$work" \
          && $CYCLONE/bin/idlc gen.idl > idlc.log 2>&1 \
          && gcc -o ref ref.c gen.c -I. -I$CYCLONE/include -L$CYCLONE/lib -lddsc \
                 -Wl,-rpath,$CYCLONE/lib > cc.log 2>&1 ) \
          || { echo "BUILD-FAILED cyclone $side"; sed -n 1,4p "$work/idlc.log" "$work/cc.log" 2>/dev/null | grep -m4 -i "error" ; return 1; }
        ;;
      rti)
        cp "$idl" "$work/gen.idl"
        cp "$dir/$frag.rti.inc" "$work/$frag.inc" 2>/dev/null \
          || cp "$dir/$frag.c.inc" "$work/$frag.inc" 2>/dev/null || { echo "no $frag.c.inc"; return 1; }
        local rep=DDS_XCDR2_DATA_REPRESENTATION
        [ "$enc" = xcdr1 ] && rep=DDS_XCDR_DATA_REPRESENTATION
        sed -e "s/@TYPE@/$type/g" -e "s/@REPRESENTATION@/$rep/g" \
            "$ROOT/tpl/rti-$( [ "$side" = writer ] && echo w.c || echo r.c )" > "$work/ref.c"
        ( cd "$work" \
          && "$NDDSHOME/bin/rtiddsgen" -language C -platform "$RTI_TARGET" -replace gen.idl > gen.log 2>&1 \
          && gcc -o ref ref.c gen.c genPlugin.c genSupport.c -I. -I$NDDSHOME/include \
                 -I$NDDSHOME/include/ndds -L$RTILIB -lnddsc -lnddscore -ldl -lm -lpthread -lrt \
                 -Wl,-rpath,$RTILIB -DRTI_UNIX -DRTI_LINUX -DRTI_64BIT > cc.log 2>&1 ) \
          || { echo "BUILD-FAILED rti $side"; grep -m4 -i "error" "$work/gen.log" "$work/cc.log" 2>/dev/null; return 1; }
        ;;
      fastdds)
        cp "$idl" "$work/gen.idl"
        cp "$dir/$frag.cpp.inc" "$work/$frag.inc" 2>/dev/null || { echo "no $frag.cpp.inc"; return 1; }
        local rep=eprosima::fastdds::dds::DataRepresentationId_t::XCDR2_DATA_REPRESENTATION
        [ "$enc" = xcdr1 ] && rep=eprosima::fastdds::dds::DataRepresentationId_t::XCDR_DATA_REPRESENTATION
        local cpptype; cpptype=$( [ -f "$dir/type.cpp" ] && cat "$dir/type.cpp" || echo "$type" )
        sed -e "s/@TYPE@/$cpptype/g" -e "s|@REPRESENTATION@|$rep|g" \
            "$ROOT/tpl/fastdds-$( [ "$side" = writer ] && echo w.cpp || echo r.cpp )" > "$work/ref.cpp"
        local extra=""
        [ "$side" = reader ] && extra="genPubSubTypes.cxx genTypeObjectSupport.cxx"
        ( cd "$work" \
          && "$FASTDDSGEN" -replace gen.idl > gen.log 2>&1 \
          && g++ -std=c++17 -o ref ref.cpp $extra -I. -I"$FASTDDS_HOME/include" -L"$FASTDDS_HOME/lib" \
                 -lfastcdr -lfastdds -Wl,-rpath,"$FASTDDS_HOME/lib" > cc.log 2>&1 ) \
          || { echo "BUILD-FAILED fastdds $side"; tail -3 "$work/gen.log" 2>/dev/null; grep -m4 "error:" "$work/cc.log" 2>/dev/null; return 1; }
        ;;
      opendds)
        cp "$idl" "$work/gen.idl"
        cp "$dir/$frag.odds.inc" "$work/$frag.inc" 2>/dev/null \
          || cp "$dir/$frag.cpp.inc" "$work/$frag.inc" 2>/dev/null || { echo "no $frag.cpp.inc"; return 1; }
        local kind=KIND_XCDR2
        [ "$enc" = xcdr1 ] && kind=KIND_XCDR1
        local cpptype; cpptype=$( [ -f "$dir/type.cpp" ] && cat "$dir/type.cpp" || echo "$type" )
        sed -e "s/@TYPE@/$cpptype/g" -e "s/@KIND@/$kind/g" \
            "$ROOT/tpl/odds-$( [ "$side" = writer ] && echo w.cpp || echo r.cpp )" > "$work/ref.cpp"
        ( cd "$work" \
          && "$TAO_IDL" -I"$OPENDDS_HOME/include" -I"$TAO_ROOT" -Sa -St --idl-version 4 gen.idl > tao.log 2>&1 \
          && "$OPENDDS_HOME/bin/opendds_idl" -I"$DDS_ROOT" --idl-version 4 gen.idl > idl.log 2>&1 \
          && g++ -std=c++17 -o ref ref.cpp genC.cpp genTypeSupportImpl.cpp -I. -I"$OPENDDS_HOME/include" \
                 -L"$OPENDDS_HOME/lib" -lOpenDDS_Dcps -lTAO_AnyTypeCode -lTAO_Valuetype -lTAO -lACE \
                 -Wl,-rpath,"$OPENDDS_HOME/lib" > cc.log 2>&1 ) \
          || { echo "BUILD-FAILED opendds $side"; grep -m3 -i error "$work/tao.log" "$work/idl.log" 2>/dev/null; grep -m4 "error:" "$work/cc.log" 2>/dev/null; return 1; }
        ;;
      omniorb)
        # omniidl is a CORBA front end and parses no annotations at all,
        # so a case that needs them supplies an annotation-free twin.
        if [ -f "$dir/$side.omni.idl" ]; then cp "$dir/$side.omni.idl" "$work/gen.idl";
        else cp "$idl" "$work/gen.idl"; fi
        cp "$dir/$frag.omni.inc" "$work/$frag.inc" 2>/dev/null \
          || cp "$dir/$frag.cpp.inc" "$work/$frag.inc" 2>/dev/null || { echo "no $frag.cpp.inc"; return 1; }
        local cpptype; cpptype=$( [ -f "$dir/type.cpp" ] && cat "$dir/type.cpp" || echo "$type" )
        sed -e "s/@TYPE@/$cpptype/g" \
            "$ROOT/tpl/omni-$( [ "$side" = writer ] && echo w.cpp || echo r.cpp )" > "$work/ref.cpp"
        ( cd "$work" \
          && "$OMNIIDL" -bcxx gen.idl > omniidl.log 2>&1 \
          && g++ -std=c++17 -o ref ref.cpp genSK.cc -I. -I"$OMNIORB_INCLUDE" \
                 -D__x86__ -D__linux__ -D__OSVERSION__=2 \
                 -lomniORB4 -lomnithread -lomniDynamic4 > cc.log 2>&1 ) \
          || { echo "BUILD-FAILED omniorb $side"; grep -m3 -i error "$work/omniidl.log" 2>/dev/null; grep -m4 "error:" "$work/cc.log" 2>/dev/null; return 1; }
        ;;
      *) echo "unknown vendor $vendor"; return 1 ;;
    esac
    echo "$work/ref"
}

case ${1:-} in
  build-writer) build "$2" "$3" writer "${4:-xcdr2}" ;;
  build-reader) build "$2" "$3" reader "${4:-xcdr2}" ;;
  write)
    bin=$(build "$2" "$3" writer "${4:-xcdr2}") || { echo "$bin"; exit 1; }
    "$bin"
    ;;
  read)
    bin=$(build "$2" "$3" reader "$4") || { echo "$bin"; exit 1; }
    printf '%s' "$5" | "$bin"
    ;;
  *) sed -n '2,12p' "$0" ;;
esac
