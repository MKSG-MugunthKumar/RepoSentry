# Test Generation Hook

This hook automatically generates comprehensive tests when new code is added or existing code is modified, ensuring consistent test coverage across the project.

## Trigger Conditions

This hook runs when:
- New Rust modules are added to src/
- Existing functions or structs are modified
- Configuration schema changes
- API endpoints are added or modified
- Critical business logic is implemented

## Hook Command

```bash
/testing-suite:generate-tests [file-path]
```

## What This Hook Does

### Comprehensive Test Generation
1. **Unit Tests**
   - Function-level testing with various inputs
   - Edge case and boundary condition testing
   - Error handling and validation testing
   - Mock implementations for external dependencies

2. **Integration Tests**
   - CLI command testing
   - Configuration loading and validation
   - End-to-end workflow testing
   - Cross-module interaction testing

3. **Property-Based Tests**
   - Generates random test cases using `quickcheck`
   - Tests invariants and properties
   - Validates configuration parsing edge cases

### Test Types Created

#### For `src/config.rs`:
- Configuration loading/saving tests
- Environment variable expansion tests
- Validation and error handling tests
- Serialization/deserialization tests

#### For `src/github.rs`:
- Authentication strategy tests
- Repository discovery and filtering tests
- API error handling tests
- Mock GitHub API response tests

#### For `src/main.rs`:
- CLI argument parsing tests
- Command execution tests
- Integration workflow tests
- Error scenario testing

### Test Infrastructure Setup
- **Mock Frameworks**: Sets up `mockall` for trait mocking
- **Test Utilities**: Creates helper functions and test data
- **Coverage Reporting**: Configures `cargo-tarpaulin` for coverage
- **CI Integration**: Updates GitHub Actions for automated testing

## Configuration

The hook can be configured in `.claude/config.yml`:

```yaml
hooks:
  generate-tests:
    auto_run: true
    trigger_on:
      - "src/**/*.rs"
    test_types:
      - unit
      - integration
      - property_based
    coverage_threshold: 80
    exclude_patterns:
      - "src/main.rs" # Skip for CLI-heavy files
```

## Manual Trigger

Generate tests for specific files:

```bash
/testing-suite:generate-tests src/config.rs
/testing-suite:generate-tests src/github.rs
/testing-suite:generate-tests  # Generate for entire project
```

## Expected Output

The hook creates:

### Unit Tests (in each module)
```rust
#[cfg(test)]
mod tests {
    use super::*;
    // Comprehensive unit tests
}
```

### Integration Tests (in `tests/` directory)
- `tests/integration_tests.rs`: CLI command testing
- `tests/common/mod.rs`: Test utilities and helpers
- `tests/config_tests.rs`: Configuration system testing
- `tests/github_tests.rs`: GitHub API integration testing

### Test Dependencies (in `Cargo.toml`)
```toml
[dev-dependencies]
tokio-test = "0.4"
mockall = "0.12"
wiremock = "0.5"
assert_matches = "1.5"
fake = "2.9"
quickcheck = "1.0"
assert_fs = "1.0"
predicates = "3.0"
```

### Coverage Configuration
- GitHub Actions workflow for automated testing
- `cargo-tarpaulin` configuration for coverage reporting
- Coverage badges and reporting setup

## Benefits

- **Consistent Quality**: Every new feature gets comprehensive tests
- **Regression Prevention**: Tests catch breaking changes early
- **Documentation**: Tests serve as usage examples
- **Confidence**: High coverage enables safe refactoring
- **CI/CD Integration**: Automated testing in development workflow

## Test Execution

After generation, run tests with:

```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --test integration_tests

# All tests with coverage
cargo tarpaulin --out html
```

This hook ensures that the codebase maintains high quality standards and comprehensive test coverage as it grows.