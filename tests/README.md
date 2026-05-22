# Test Suite for nagios-plugin-mongodb

This directory contains a comprehensive test suite that tests both the original Python implementation (`check_mongodb.py`) and the Rust rewrite (`src/main.rs`).

## Test Suite Overview

The test suite runs both implementations through a series of tests to verify:
- Help and version output
- Error handling for invalid actions
- Connection failure handling (non-existent hosts/ports)
- Argument parsing (ports, credentials, thresholds, database/collection names)
- Performance data flags
- Various action handlers

## Running the Tests

### Full Test Suite

Run all tests for both Python and Rust implementations:

```bash
python3 tests/test_suite.py
```

### Python Only

Run only the Python implementation tests:

```bash
python3 tests/test_suite.py --python
```

### Rust Only

Run only the Rust implementation tests:

```bash
python3 tests/test_suite.py --rust
```

### With Verbose Output

Show detailed output for all tests:

```bash
python3 tests/test_suite.py -v
```

### Filter Tests

Run tests matching a specific pattern:

```bash
python3 tests/test_suite.py --filter "connect"
```

## Test Categories

1. **Help/Version Tests**: Verify help text is displayed correctly
2. **Invalid Action Tests**: Verify proper error handling for unsupported actions
3. **Connection Failure Tests**: Test behavior when MongoDB is unreachable
4. **Argument Parsing Tests**: Verify command-line arguments are parsed correctly
5. **Performance Data Tests**: Test the `-D` flag for performance data output
6. **Action Tests**: Test various supported actions (connect, connections, databases, collections, memory)

## Requirements

- Python 3.x
- The Rust binary must be built (`cargo build`)
- Both `check_mongodb.py` and the Rust binary must be in their expected locations

## Exit Codes

The test suite returns:
- `0`: All tests passed
- `1`: At least one test failed

## Adding New Tests

Add new test cases in the `create_test_cases()` function in `test_suite.py`. Each test is a `TestResult` object with:
- `name`: Descriptive test name
- `implementation`: Path to the implementation (PYTHON_PLUGIN or RUST_BINARY)
- `args`: List of command-line arguments
- `expected_exit_code`: Expected Nagios exit code (0=OK, 1=WARNING, 2=CRITICAL, 3=UNKNOWN)
- `should_contain`: List of strings that must appear in output
- `should_not_contain`: List of strings that must NOT appear in output
- `timeout`: Test timeout in seconds

## Notes

- Tests use port 27999 and host 192.0.2.1 (TEST-NET-1) which should not have MongoDB running
- Tests are designed to work without a live MongoDB instance
- Connection timeout tests may take longer due to DNS resolution
