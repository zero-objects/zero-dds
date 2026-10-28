# Native D endpoint: byte-identity + sync + async (gdc + phobos/std.concurrency).
DC ?= gdc

.PHONY: test example example_sync example_async clean \
        reliable_test example_reliable reliable_app reliable_bench
# surface_test.d is compiled in and not run apart: `make test` is the one
# command CI runs, and a module nothing links goes unmeasured.
test:
	$(DC) zerodds.d surface_test.d xrce_client.d xrce_client_test.d test.d -o test_bin
	GOLDEN_DIR=$(GOLDEN_DIR) ./test_bin

# The reliable half of the committed suite. These files sat beside the
# Makefile with no target naming them, which is the ada-native defect by
# another route: a suite nothing runs is documentation.
reliable_test:
	$(DC) reliable.d reliable_test.d -o reliable_test_bin
	./reliable_test_bin $(GOLDEN_DIR)/golden_heartbeat_le.bin $(GOLDEN_DIR)/golden_acknack_le.bin

example_reliable:
	$(DC) reliable.d example_reliable.d -o example_reliable_bin
	./example_reliable_bin

# reliable_app takes a live peer (port + count), so it is compiled and
# not run here.
reliable_app:
	$(DC) reliable.d reliable_app.d -o reliable_app_bin

reliable_bench:
	$(DC) reliable.d reliable_bench.d -o reliable_bench_bin
	./reliable_bench_bin

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
	rm -f *.o test_bin reliable_test_bin example_reliable_bin \
	      reliable_app_bin reliable_bench_bin \
	      example_bin example_sync_bin example_async_bin
