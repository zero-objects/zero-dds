# Native OCaml endpoint: byte-identity + sync + async (threads.posix, no Lwt).
# -thread is required on OCaml 4.x (Mutex/Condition/Thread live in the threads
# library there; on 5.x they are in the stdlib, where -thread is harmless).
OCAMLFIND = ocamlfind ocamlopt -thread -package threads.posix -linkpkg

.PHONY: test example example_sync example_async clean
test:
	$(OCAMLFIND) zerodds.ml test.ml -o test_bin
	GOLDEN_DIR=$(GOLDEN_DIR) ./test_bin

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
	rm -f *.cm* *.o test_bin example_bin example_sync_bin example_async_bin
