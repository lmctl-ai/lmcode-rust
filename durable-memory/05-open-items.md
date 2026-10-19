# Open items

1. Publish or tag the crate (`v4.0.0`) once a host team depends on it by git; today path dependencies on a sibling checkout.
2. Intern type names (`Rc<str>`) and avoid `String` clones on `PUSH_STR` if a host measures the VM as a cost (it is not today).
3. When spec v4 lands (lists, overloads; design round in lmctl-mono `proposals/dsl-v4-design-*.md`): bump `VERSION`, add opcodes
   to `Op::decode`, the verifier and `run`, resync the vectors, keep the C++ and Rust message texts aligned.
4. A Rust host of a real library (an lmrust tool) to exercise handles with real ownership (the router's objects are trivial).
5. Add the UTF-8-constant rule to the living spec and to the C++ header (or drop it here) so both runtimes agree on invalid files.
