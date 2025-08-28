# Nyx Protocol Compliance Testing Makefile
#
# This Makefile provides convenient targets for running compliance tests
# and generating compliance reports during development.

.PHONY: help compliance-check compliance-report compliance-badges compliance-ci \
        compliance-full test-core test-plus test-full test-matrix clean

# Default target
help:
	@echo "Nyx Protocol Compliance Testing"
	@echo "================================"
	@echo ""
	@echo "Available targets:"
	@echo "  compliance-check    - Run core compliance gate (required for all builds)"
	@echo "  compliance-report   - Generate detailed compliance reports"
	@echo "  compliance-badges   - Generate compliance badges and documentation"
	@echo "  compliance-ci       - Run full CI/CD compliance test suite"
	@echo "  compliance-full     - Run all compliance tests with full feature set"
	@echo ""
	@echo "Individual level testing:"
	@echo "  test-core          - Test Core compliance level"
	@echo "  test-plus          - Test Plus compliance level (with more features)"
	@echo "  test-full          - Test Full compliance level (all features)"
	@echo "  test-matrix        - Run comprehensive compliance matrix"
	@echo ""
	@echo "Utilities:"
	@echo "  clean              - Clean up generated reports and artifacts"
	@echo "  hybrid-tests       - Run nyx-crypto hybrid-handshake tests"
	@echo ""
	@echo "Environment variables:"
	@echo "  NYX_REQUIRED_COMPLIANCE_LEVEL - Required compliance level (core|plus|full)"
	@echo "  NYX_CI_OUTPUT_DIR            - Output directory for reports"

# Core compliance gate - mandatory for all builds
compliance-check:
	@echo "🔍 Running Core Compliance Gate..."
	@export NYX_REQUIRED_COMPLIANCE_LEVEL=core && \
	cargo test --package nyx-conformance ci_compliance_gate_main --features hybrid -- --nocapture

# Generate detailed compliance reports
compliance-report:
	@echo "📊 Generating Compliance Reports..."
	@mkdir -p ./compliance-reports
	@export NYX_CI_OUTPUT_DIR=./compliance-reports && \
	cargo test --package nyx-conformance ci_compliance_matrix_full --features hybrid,multipath,telemetry -- --nocapture
	@echo "📁 Reports generated in: ./compliance-reports/"

# Generate compliance badges for documentation
compliance-badges:
	@echo "🏷️  Generating Compliance Badges..."
	@mkdir -p ./badges
	@export NYX_CI_OUTPUT_DIR=./badges && \
	cargo test --package nyx-conformance ci_compliance_matrix_full --features hybrid,multipath,telemetry -- --nocapture
	@echo "📁 Badges generated in: ./badges/"
	@if [ -f ./badges/compliance_badges.md ]; then \
		echo "📄 Badge markdown:"; \
		cat ./badges/compliance_badges.md; \
	fi

# Full CI/CD compliance test suite
compliance-ci:
	@echo "🧪 Running Full CI/CD Compliance Suite..."
	@mkdir -p ./compliance-reports/ci
	@export NYX_CI_OUTPUT_DIR=./compliance-reports/ci && \
	echo "Running Core Compliance Gate..." && \
	export NYX_REQUIRED_COMPLIANCE_LEVEL=core && \
	cargo test --package nyx-conformance ci_compliance_gate_main --features hybrid -- --nocapture && \
	echo "Running Compliance Matrix..." && \
	cargo test --package nyx-conformance ci_compliance_matrix_full --features hybrid,multipath,telemetry -- --nocapture && \
	echo "Running Feature Compilation Verification..." && \
	cargo test --package nyx-conformance ci_feature_compilation_verification --features hybrid,multipath,telemetry -- --nocapture && \
	echo "Running Hierarchy Validation..." && \
	cargo test --package nyx-conformance ci_compliance_hierarchy_validation --features hybrid,multipath,telemetry -- --nocapture
	@echo "✅ CI/CD Compliance Suite completed"

# Run all compliance tests with full feature set
compliance-full:
	@echo "🚀 Running Full Compliance Test Suite..."
	@mkdir -p ./compliance-reports/full
	@export NYX_CI_OUTPUT_DIR=./compliance-reports/full && \
	cargo test --package nyx-conformance --features hybrid,multipath,telemetry,plugin,quic -- --nocapture
	@echo "📁 Full compliance reports in: ./compliance-reports/full/"

# Test individual compliance levels
test-core:
	@echo "🔐 Testing Core Compliance Level..."
	@export NYX_REQUIRED_COMPLIANCE_LEVEL=core && \
	cargo test --package nyx-conformance ci_compliance_gate_main --features hybrid -- --nocapture

test-plus:
	@echo "⚡ Testing Plus Compliance Level..."
	@export NYX_REQUIRED_COMPLIANCE_LEVEL=plus && \
	cargo test --package nyx-conformance ci_compliance_gate_main --features hybrid,multipath,telemetry || true
	@echo "ℹ️  Plus compliance may not be fully achievable with current feature set"

test-full:
	@echo "🌟 Testing Full Compliance Level..."
	@export NYX_REQUIRED_COMPLIANCE_LEVEL=full && \
	cargo test --package nyx-conformance ci_compliance_gate_main --features hybrid,multipath,telemetry,plugin,quic || true
	@echo "ℹ️  Full compliance may not be fully achievable with current feature set"

# Run comprehensive compliance matrix
test-matrix:
	@echo "🧪 Running Comprehensive Compliance Matrix..."
	@mkdir -p ./compliance-reports/matrix
	@export NYX_CI_OUTPUT_DIR=./compliance-reports/matrix && \
	echo "Testing with minimal features..." && \
	cargo test --package nyx-conformance ci_compliance_matrix_full --features hybrid -- --nocapture && \
	echo "Testing with expanded features..." && \
	cargo test --package nyx-conformance ci_compliance_matrix_full --features hybrid,multipath,telemetry -- --nocapture
	@echo "📊 Matrix testing completed"

# Clean up generated artifacts
clean:
	@echo "🧹 Cleaning up compliance artifacts..."
	@rm -rf ./compliance-reports ./badges
	@cargo clean --package nyx-conformance
	@echo "✅ Cleanup completed"

# Hybrid handshake tests (nyx-crypto)
.PHONY: hybrid-tests
hybrid-tests:
	@echo "🔐 Running nyx-crypto hybrid-handshake tests..."
	@bash ./scripts/run-hybrid-tests.sh
	@echo "✅ Hybrid tests completed"

# Development shortcuts
.PHONY: dev-check dev-report dev-badges

# Quick development compliance check
dev-check: compliance-check
	@echo "✅ Development compliance check passed"

# Generate development reports
dev-report: compliance-report
	@if [ -f ./compliance-reports/compliance_matrix.json ]; then \
		echo "📊 Quick compliance summary:"; \
		cat ./compliance-reports/compliance_matrix.json | jq -r '.highest_level' | sed 's/^/   Highest Level: /'; \
		cat ./compliance-reports/compliance_matrix.json | jq -r '.matrix | keys[]' | sed 's/^/   Available: /' | tr '\n' ' '; \
		echo ""; \
	fi

# Generate and display badges for development
dev-badges: compliance-badges
	@echo "🏷️  Development badge status ready"

# Integration test targets
.PHONY: test-regression test-hierarchy test-features

# Test for compliance regressions
test-regression:
	@echo "🚨 Running Regression Detection..."
	@cargo test --package nyx-conformance test_compliance_regression_detection --features hybrid,multipath,telemetry -- --nocapture

# Test compliance hierarchy validation
test-hierarchy:
	@echo "📊 Running Hierarchy Validation..."
	@cargo test --package nyx-conformance test_compliance_level_progression --features hybrid,multipath,telemetry -- --nocapture

# Test feature compilation gates
test-features:
	@echo "🔧 Running Feature Compilation Tests..."
	@cargo test --package nyx-conformance ci_feature_compilation_verification --features hybrid,multipath,telemetry -- --nocapture

# Cross-platform testing (for local development)
.PHONY: test-cross-platform

test-cross-platform:
	@echo "🌐 Running Cross-Platform Compliance Check..."
	@echo "Current platform: $$(uname -s)"
	@$(MAKE) compliance-check
	@echo "✅ Cross-platform compliance verified for current platform"

# Documentation targets
.PHONY: docs-compliance docs-update

# Generate compliance documentation
docs-compliance:
	@echo "📚 Generating Compliance Documentation..."
	@$(MAKE) compliance-badges
	@if [ -f ./badges/compliance_badges.md ]; then \
		cp ./badges/compliance_badges.md ./docs/compliance_status.md; \
		echo "📄 Compliance status documentation updated"; \
	fi

# Update all documentation
docs-update: docs-compliance
	@echo "📝 Updating all compliance documentation..."
	@echo "ℹ️  Review ./docs/compliance_ci_integration.md for integration details"
	@echo "ℹ️  Review ./docs/compliance_status.md for current status"
