set -u
cd /root/m1-bitbound
echo "### tree"; find . -type f | head -40
echo "### fastddsgen bb8 rerun, explicit outdir"
mkdir -p out8 out16
/root/vendors/fastddsgen/scripts/fastddsgen -replace -d /root/m1-bitbound/out8 /root/m1-bitbound/bb8.idl 2>&1 | tail -3
/root/vendors/fastddsgen/scripts/fastddsgen -replace -d /root/m1-bitbound/out16 /root/m1-bitbound/bb16.idl 2>&1 | tail -3
echo "### out8 files"; ls out8
echo "### enum decl in out8"
grep -n -A6 -B2 "enum Colour" out8/*.hpp | head -60
echo "### bit_bound traces out8"
grep -rn "bit_bound\|bitbound\|BitBound\|uint8_t Colour\|int8_t" out8/*.hpp | head -20
echo "### serializer for Colour, out8"
grep -rn "Colour" out8/*Cdr*.ipp out8/*CdrAux* out8/*.cxx 2>/dev/null | head -30
echo "### enum decl in out16"
grep -n -A6 -B2 "enum Colour" out16/*.hpp | head -40
