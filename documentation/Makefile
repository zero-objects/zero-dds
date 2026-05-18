# documentation/Makefile — build PDFs and API references.
#
# Two backends:
# * Markdown → PDF via pandoc + tectonic (each station)
# * Direct LaTeX → PDF via tectonic (architecture, vendor specs)
#
# Usage:
#   make pdfs            # all station PDFs
#   make pdf-arch        # the LaTeX-authored architecture PDF
#   make pdf-getting-started, pdf-config, pdf-idl, pdf-integration,
#   pdf-operations, pdf-migration
#   make pdf-specs       # the vendor specs (DDS-AMQP 1.0, DDS-TS 1.0)
#   make api             # generate API reference for all bindings
#   make clean

DIST     := dist
TEX      := tectonic
TEXOPTS  := --keep-logs --keep-intermediates
PANDOC   := pandoc
PANDOC_OPTS := \
  --standalone \
  --pdf-engine=tectonic \
  --template=template.tex \
  --variable=toc:true \
  --metadata=title

# Per-station markdown-to-PDF targets. Each target collects every .md
# file in the station directory (except the README is the lead) into
# a single PDF.
STATIONS := getting-started config idl integration operations migration

.PHONY: pdfs pdf-arch pdf-specs api clean help \
        $(addprefix pdf-,$(STATIONS)) \
        $(addprefix api-,rust c cpp java python typescript csharp)

help:
	@echo "ZeroDDS documentation Makefile."
	@echo ""
	@echo "PDF targets:"
	@echo "  pdfs                 — every station PDF + architecture + vendor specs"
	@echo "  pdf-arch             — architecture (LaTeX-authored)"
	@for s in $(STATIONS); do \
	  echo "  pdf-$$s$$(printf '%*s' $$((20-$${#s})) '') — $$s station"; \
	done
	@echo "  pdf-specs            — DDS-AMQP 1.0 + DDS-TS 1.0 vendor specs"
	@echo ""
	@echo "API reference:"
	@echo "  api                  — every language"
	@echo "  api-{rust,c,cpp,java,python,typescript,csharp}"
	@echo ""
	@echo "  clean                — remove dist/"

pdfs: pdf-arch \
      pdf-getting-started pdf-config pdf-idl pdf-integration \
      pdf-operations pdf-migration \
      pdf-specs

# ---------------------------------------------------------------------
# Station PDFs via pandoc.
# ---------------------------------------------------------------------

$(DIST):
	@mkdir -p $(DIST)

# Concatenate every .md in the station directory in lexicographic order
# (README first since alphabet, then sub-pages). Pandoc-aware
# Markdown handles the resulting concatenation cleanly.
define STATION_PDF_RULE
pdf-$(1): $(DIST)/$(1).pdf
$(DIST)/$(1).pdf: 0$(if $(filter getting-started,$(1)),1,$(if $(filter config,$(1)),3,$(if $(filter idl,$(1)),4,$(if $(filter integration,$(1)),5,$(if $(filter operations,$(1)),6,$(if $(filter migration,$(1)),7,)))))-$(1)/README.md | $(DIST)
	$$(PANDOC) $$(PANDOC_OPTS) "ZeroDDS — $(1)" \
	  $$(sort $$(wildcard 0$$(if $$(filter getting-started,$(1)),1,$$(if $$(filter config,$(1)),3,$$(if $$(filter idl,$(1)),4,$$(if $$(filter integration,$(1)),5,$$(if $$(filter operations,$(1)),6,$$(if $$(filter migration,$(1)),7,))))))-$(1)/*.md)) \
	  -o $$@
endef

# Cheaper: write each rule inline rather than via expand-trickery.
$(DIST)/getting-started.pdf: 01-getting-started/README.md \
                              01-getting-started/installation.md \
                              01-getting-started/concepts.md \
                              01-getting-started/first-publisher.md | $(DIST)
	$(PANDOC) $(PANDOC_OPTS) "Getting Started" $^ -o $@

pdf-getting-started: $(DIST)/getting-started.pdf

$(DIST)/config.pdf: 03-configuration/README.md \
                     03-configuration/runtime-config.md \
                     03-configuration/qos-policies.md \
                     03-configuration/transport.md \
                     03-configuration/security.md \
                     03-configuration/observability.md | $(DIST)
	$(PANDOC) $(PANDOC_OPTS) "Configuration" $^ -o $@

pdf-config: $(DIST)/config.pdf

$(DIST)/idl.pdf: 04-idl/README.md \
                  04-idl/language.md \
                  04-idl/annotations.md \
                  04-idl/codegen.md | $(DIST)
	$(PANDOC) $(PANDOC_OPTS) "IDL Reference" $^ -o $@

pdf-idl: $(DIST)/idl.pdf

$(DIST)/integration.pdf: 05-integration/README.md \
                          05-integration/rust.md \
                          05-integration/c.md \
                          05-integration/cpp.md \
                          05-integration/csharp.md \
                          05-integration/java.md \
                          05-integration/python.md \
                          05-integration/typescript-node.md \
                          05-integration/typescript-wasm.md \
                          05-integration/ros2.md | $(DIST)
	$(PANDOC) $(PANDOC_OPTS) "Integration per Language" $^ -o $@

pdf-integration: $(DIST)/integration.pdf

$(DIST)/operations.pdf: 06-operations/README.md \
                         06-operations/deployment.md \
                         06-operations/monitoring.md \
                         06-operations/troubleshooting.md | $(DIST)
	$(PANDOC) $(PANDOC_OPTS) "Operations" $^ -o $@

pdf-operations: $(DIST)/operations.pdf

$(DIST)/migration.pdf: 07-migration/README.md \
                        07-migration/from-cyclonedds.md \
                        07-migration/from-fastdds.md \
                        07-migration/from-rti-connext.md \
                        07-migration/from-opendds.md | $(DIST)
	$(PANDOC) $(PANDOC_OPTS) "Migration Guides" $^ -o $@

pdf-migration: $(DIST)/migration.pdf

# ---------------------------------------------------------------------
# Architecture (LaTeX-authored, bypasses pandoc).
# ---------------------------------------------------------------------

pdf-arch: $(DIST)/architecture.pdf

$(DIST)/architecture.pdf: 02-architecture/architecture.tex | $(DIST)
	$(TEX) $(TEXOPTS) --outdir=$(DIST) $<

# ---------------------------------------------------------------------
# Vendor-spec PDFs (LaTeX projects under specs/).
# ---------------------------------------------------------------------

pdf-specs: $(DIST)/dds-amqp-1.0.pdf $(DIST)/dds-ts-1.0.pdf

$(DIST)/dds-amqp-1.0.pdf: specs/dds-amqp-1.0/main.tex | $(DIST)
	$(TEX) $(TEXOPTS) --outdir=$(DIST) $<
	@if [ -f $(DIST)/main.pdf ]; then mv $(DIST)/main.pdf $@; fi

$(DIST)/dds-ts-1.0.pdf: specs/dds-ts-1.0/main.tex | $(DIST)
	$(TEX) $(TEXOPTS) --outdir=$(DIST) $<
	@if [ -f $(DIST)/main.pdf ]; then mv $(DIST)/main.pdf $@; fi

# ---------------------------------------------------------------------
# API reference (per language).
# ---------------------------------------------------------------------

api:
	./api/gen.sh

api-rust:
	./api/gen.sh rust
api-c:
	./api/gen.sh c
api-cpp:
	./api/gen.sh cpp
api-java:
	./api/gen.sh java
api-python:
	./api/gen.sh python
api-typescript:
	./api/gen.sh typescript
api-csharp:
	./api/gen.sh csharp

# ---------------------------------------------------------------------

clean:
	rm -rf $(DIST)
	rm -rf api/rust api/c api/cpp api/java api/python api/typescript api/csharp
