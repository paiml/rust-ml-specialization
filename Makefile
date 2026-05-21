# Rust for Machine Learning — 56-course Coursera Professional Certificate
#
# This repo is a specification + scaffold. Actual rendering of course videos
# is owned by ../course-studio (Lua configs) + ../rmedia (Rust renderer).

.DEFAULT_GOAL := help
SHELL := /bin/bash

# ----------------------------------------------------------------------------
# Discovery
# ----------------------------------------------------------------------------

TRACK_DIRS  := $(sort $(wildcard courses/track-*))
COURSE_DIRS := $(sort $(wildcard courses/track-*/course-*))

# ----------------------------------------------------------------------------
# Targets
# ----------------------------------------------------------------------------

.PHONY: help
help: ## Show this help
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

.PHONY: lint
lint: ## Lint markdown files (requires markdownlint-cli or npx)
	@if command -v markdownlint >/dev/null 2>&1; then \
		markdownlint --disable MD013 MD033 MD041 -- README.md courses; \
	elif command -v npx >/dev/null 2>&1; then \
		npx --yes markdownlint-cli --disable MD013 MD033 MD041 -- README.md courses; \
	else \
		echo "markdownlint not installed; skipping lint"; \
	fi

.PHONY: test
test: test-tracks test-courses test-capstone ## Run all structural tests

.PHONY: test-tracks
test-tracks: ## Verify 8 track directories exist
	@n=$$(ls -d courses/track-* 2>/dev/null | wc -l); \
	if [ "$$n" != "8" ]; then \
		echo "FAIL: expected 8 tracks, found $$n"; exit 1; \
	fi; \
	echo "OK: 8 tracks present"

.PHONY: test-courses
test-courses: ## Verify 56 course directories exist (7 per track)
	@n=$$(ls -d courses/track-*/course-* 2>/dev/null | wc -l); \
	if [ "$$n" != "56" ]; then \
		echo "FAIL: expected 56 courses, found $$n"; exit 1; \
	fi; \
	for t in $(TRACK_DIRS); do \
		c=$$(ls -d $$t/course-* 2>/dev/null | wc -l); \
		if [ "$$c" != "7" ]; then \
			echo "FAIL: $$t has $$c courses (expected 7)"; exit 1; \
		fi; \
	done; \
	echo "OK: 56 courses (7 per track) present"

.PHONY: test-capstone
test-capstone: ## Verify every course README mentions a Capstone section
	@missing=0; \
	for d in $(COURSE_DIRS); do \
		if [ ! -f "$$d/README.md" ]; then \
			echo "FAIL: $$d/README.md missing"; missing=$$((missing+1)); \
		elif ! grep -qiE '^## .*Capstone' "$$d/README.md"; then \
			echo "FAIL: $$d/README.md missing '## Capstone' section"; missing=$$((missing+1)); \
		fi; \
	done; \
	if [ "$$missing" != "0" ]; then exit 1; fi; \
	echo "OK: all 56 courses have a Capstone section"

.PHONY: check
check: lint test ## Run lint + test

.PHONY: clean
clean: ## Remove build artifacts (none expected; placeholder for parity)
	@true
