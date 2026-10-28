# Native OCaml endpoint: byte-identity + sync + async (threads.posix, no Lwt).
# -thread is required on OCaml 4.x (Mutex/Condition/Thread live in the threads
# library there; on 5.x they are in the stdlib, where -thread is harmless).
OCAMLFIND = ocamlfind ocamlopt -thread -package threads.posix -linkpkg

.PHONY: test surface_test xrce_client_test example example_sync example_async clean
test:
	$(OCAMLFIND) zerodds.ml test.ml -o test_bin
	GOLDEN_DIR=$(GOLDEN_DIR) ./test_bin

# The DDS-XRCE 1.0 client: wire-encoder byte-identity + state-machine
# lifecycle rules, ported from crates/xrce-client.
xrce_client_test:
	$(OCAMLFIND) zerodds.ml reliable.ml xrce_client.ml xrce_client_test.ml -o xrce_client_test_bin
	./xrce_client_test_bin

# The XCDR surface against the vendors' own octets. `-warn-error +a` is the
# line tools/wireprobe/endpoint-common.sh compiles this column with, so a
# warning here is a warning the corpus sweep would have failed on too.
surface_test:
	$(OCAMLFIND) -w +a-4-9-40-41-42-44-45-70 -warn-error +a \
		zerodds.ml surface_test.ml -o surface_test_bin
	./surface_test_bin

example:
	$(OCAMLFIND) zerodds.ml example.ml -o example_bin
	./example_bin

example_sync:
	$(OCAMLFIND) zerodds.ml example_sync.ml -o example_sync_bin
	./example_sync_bin

example_async:
	$(OCAMLFIND) zerodds.ml example_async.ml -o example_async_bin
	./example_async_bin

clean:
	rm -f *.cm* *.o test_bin surface_test_bin xrce_client_test_bin \
	      example_bin example_sync_bin example_async_bin
