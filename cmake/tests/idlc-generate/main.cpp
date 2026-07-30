// Trivial consumer that links the generated INTERFACE library. It exists only
// to give the build graph a concrete target to build, which drives IDL
// generation through the library's dependency on <lib>_idlc_generate. It does
// not #include the generated header, so the test stays hermetic (no zerodds C++
// runtime headers required).
int main() {
    return 0;
}
