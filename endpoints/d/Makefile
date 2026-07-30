# Native D endpoint: byte-identity + sync + async (gdc + phobos/std.concurrency).
DC ?= gdc

.PHONY: test example example_sync example_async clean
test:
	$(DC) zerodds.d test.d -o test_bin
	GOLDEN_DIR=$(GOLDEN_DIR) ./test_bin

example:
	$(DC) zerodds.d example.d -o example_bin
	./example_bin

example_sync:
	$(DC) zerodds.d example_sync.d -o example_sync_bin
	./example_sync_bin

example_async:
	$(DC) zerodds.d example_async.d -o example_async_bin
	./example_async_bin

clean:
	rm -f *.o test_bin example_bin example_sync_bin example_async_bin
