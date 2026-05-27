#!/usr/bin/env python3
"""
Test suite for nagios-plugin-mongodb
Tests both the original Python implementation and the Rust rewrite
"""

import subprocess
import sys
import os
from pathlib import Path

# Paths
PROJECT_ROOT = Path(__file__).parent.parent
PYTHON_PLUGIN = PROJECT_ROOT / "check_mongodb.py"
RUST_BINARY = PROJECT_ROOT / "target" / "debug" / "check_mongodb"

# Use a port that's unlikely to have MongoDB running
NON_EXISTENT_PORT = 27999
NON_EXISTENT_HOST = "192.0.2.1"

class TestResult:
    def __init__(self, name, implementation, args, expected_exit_code=None,
                 should_contain=None, should_not_contain=None, timeout=10):
        self.name = name
        self.implementation = implementation
        self.args = args
        self.expected_exit_code = expected_exit_code
        self.should_contain = should_contain or []
        self.should_not_contain = should_not_contain or []
        self.timeout = timeout
        self.actual_exit_code = None
        self.actual_output = ""
        self.actual_error = ""
        self.passed = False
        self.error = None

    def run(self):
        """Run the test and return True if passed"""
        cmd = [str(self.implementation)] + self.args

        try:
            result = subprocess.run(
                cmd,
                timeout=self.timeout,
                capture_output=True,
                text=True
            )
            self.actual_exit_code = result.returncode
            self.actual_output = result.stdout
            self.actual_error = result.stderr

            # Check exit code
            if self.expected_exit_code is not None:
                if self.actual_exit_code != self.expected_exit_code:
                    self.error = f"Expected exit code {self.expected_exit_code}, got {self.actual_exit_code}"
                    return False

            # Check output contains expected strings
            combined_output = (self.actual_output + self.actual_error).lower()
            for text in self.should_contain:
                if text.lower() not in combined_output:
                    self.error = f"Expected output to contain '{text}', but it didn't. Output: {combined_output[:200]}"
                    return False

            # Check output doesn't contain unexpected strings
            for text in self.should_not_contain:
                if text.lower() in combined_output:
                    self.error = f"Expected output NOT to contain '{text}', but it did"
                    return False

            self.passed = True
            return True

        except subprocess.TimeoutExpired:
            self.error = f"Test timed out after {self.timeout} seconds"
            self.actual_exit_code = -1
            return False
        except FileNotFoundError as e:
            self.error = f"Implementation not found: {e}"
            self.actual_exit_code = -1
            return False
        except Exception as e:
            self.error = f"Error running test: {e}"
            self.actual_exit_code = -1
            return False


def create_test_cases():
    """Create test cases for both implementations"""
    tests = []

    # ============================================
    # Help and Version Tests
    # ============================================

    tests.append(TestResult(
        name="Python: --help",
        implementation=PYTHON_PLUGIN,
        args=["--help"],
        expected_exit_code=0,
        should_contain=["usage", "mongodb"]
    ))

    tests.append(TestResult(
        name="Rust: --help",
        implementation=RUST_BINARY,
        args=["--help"],
        expected_exit_code=0,
        should_contain=["usage", "mongodb"]
    ))

    # Python: invalid action - optparse exits with code 2 and prints error to stderr
    tests.append(TestResult(
        name="Python: invalid action",
        implementation=PYTHON_PLUGIN,
        args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-A", "nonexistent_action", "-t", "2"],
        expected_exit_code=2,
        should_contain=["error", "invalid choice"]  # optparse error message
    ))

    # Rust: invalid action
    tests.append(TestResult(
        name="Rust: invalid action",
        implementation=RUST_BINARY,
        args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-A", "nonexistent_action"],
        expected_exit_code=1,
        should_contain=["WARNING", "not yet implemented"]
    ))

    # ============================================
    # Connection Failure Tests
    # ============================================

    # Test connection to non-existent host (Python uses longer timeout for DNS resolution)
    tests.append(TestResult(
        name="Python: connect to non-existent host",
        implementation=PYTHON_PLUGIN,
        args=["-H", NON_EXISTENT_HOST, "-P", "27017", "-A", "connect", "-t", "3"],
        expected_exit_code=2,
        should_contain=["CRITICAL"],
        timeout=15  # DNS resolution can take longer
    ))

    tests.append(TestResult(
        name="Rust: connect to non-existent host",
        implementation=RUST_BINARY,
        args=["-H", NON_EXISTENT_HOST, "-P", "27017", "-A", "connect", "-t", "2"],
        expected_exit_code=2,
        should_contain=["CRITICAL"],
        timeout=5
    ))

    # Test connection to non-existent port on localhost
    tests.append(TestResult(
        name="Python: connect to non-existent port",
        implementation=PYTHON_PLUGIN,
        args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-A", "connect", "-t", "2"],
        expected_exit_code=2,
        should_contain=["CRITICAL"],
        timeout=5
    ))

    tests.append(TestResult(
        name="Rust: connect to non-existent port",
        implementation=RUST_BINARY,
        args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-A", "connect", "-t", "2"],
        expected_exit_code=2,
        should_contain=["CRITICAL"],
        timeout=5
    ))

    # ============================================
    # Argument Parsing Tests
    # ============================================

    # Test with various ports (will fail to connect but should parse args correctly)
    tests.append(TestResult(
        name="Python: custom port",
        implementation=PYTHON_PLUGIN,
        args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-A", "connect", "-t", "2"],
        expected_exit_code=2
    ))

    tests.append(TestResult(
        name="Rust: custom port",
        implementation=RUST_BINARY,
        args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-A", "connect", "-t", "2"],
        expected_exit_code=2
    ))

    # Test with username/password
    tests.append(TestResult(
        name="Python: with credentials",
        implementation=PYTHON_PLUGIN,
        args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-u", "testuser", "-p", "testpass",
              "-A", "connect", "-t", "2"],
        expected_exit_code=2,
        timeout=5
    ))

    tests.append(TestResult(
        name="Rust: with credentials",
        implementation=RUST_BINARY,
        args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-u", "testuser", "-p", "testpass",
              "-A", "connect", "-t", "2"],
        expected_exit_code=2,
        timeout=5
    ))

    # Test with warning/critical thresholds
    tests.append(TestResult(
        name="Python: with thresholds",
        implementation=PYTHON_PLUGIN,
        args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-A", "connections",
              "-W", "80", "-C", "95", "-t", "2"],
        expected_exit_code=2,
        timeout=5
    ))

    tests.append(TestResult(
        name="Rust: with thresholds",
        implementation=RUST_BINARY,
        args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-A", "connections",
              "-W", "80", "-C", "95", "-t", "2"],
        expected_exit_code=2,
        timeout=5
    ))

    # ============================================
    # Performance Data Tests
    # ============================================

    tests.append(TestResult(
        name="Python: perf data flag",
        implementation=PYTHON_PLUGIN,
        args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-A", "connect", "-D", "-t", "2"],
        expected_exit_code=2,
        timeout=5
    ))

    tests.append(TestResult(
        name="Rust: perf data flag",
        implementation=RUST_BINARY,
        args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-A", "connect", "-D", "-t", "2"],
        expected_exit_code=2,
        timeout=5
    ))

    # ============================================
    # Database/Collection argument tests
    # ============================================

    tests.append(TestResult(
        name="Python: with database arg",
        implementation=PYTHON_PLUGIN,
        args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-d", "testdb",
              "-A", "database_size", "-t", "2"],
        expected_exit_code=2,
        timeout=5
    ))

    tests.append(TestResult(
        name="Rust: with database arg",
        implementation=RUST_BINARY,
        args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-d", "testdb",
              "-A", "database_size", "-t", "2"],
        expected_exit_code=2,
        timeout=5
    ))

    tests.append(TestResult(
        name="Python: with collection arg",
        implementation=PYTHON_PLUGIN,
        args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-d", "testdb", "-c", "testcoll",
              "-A", "collection_state", "-t", "2"],
        expected_exit_code=2,
        timeout=5
    ))

    tests.append(TestResult(
        name="Rust: with collection arg",
        implementation=RUST_BINARY,
        args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-d", "testdb", "-c", "testcoll",
              "-A", "collection_state", "-t", "2"],
        expected_exit_code=2,
        timeout=5
    ))

    # ============================================
    # Various Action Tests
    # ============================================

    # Test simple actions that should work without a live MongoDB (will fail to connect but test parsing)
    actions = ["connect", "connections", "replset_state", "replset_quorum", "memory",
              "memory_mapped", "lock", "flushing", "database_size", "database_indexes"]

    for action in actions:
        tests.append(TestResult(
            name=f"Python: action={action}",
            implementation=PYTHON_PLUGIN,
            args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-A", action, "-t", "1"],
            expected_exit_code=2,
            timeout=3
        ))

        tests.append(TestResult(
            name=f"Rust: action={action}",
            implementation=RUST_BINARY,
            args=["-H", "127.0.0.1", "-P", str(NON_EXISTENT_PORT), "-A", action, "-t", "1"],
            expected_exit_code=2,
            timeout=3
        ))

    return tests


def print_results(tests, verbose=False):
    """Print test results in a nice format"""
    print("\n" + "=" * 80)
    print("TEST SUITE RESULTS")
    print("=" * 80)

    # Summary by implementation
    python_tests = [t for t in tests if "Python" in t.name]
    rust_tests = [t for t in tests if "Rust" in t.name]

    python_passed = sum(1 for t in python_tests if t.passed)
    rust_passed = sum(1 for t in rust_tests if t.passed)

    print(f"\nPython ({PYTHON_PLUGIN.name}): {python_passed}/{len(python_tests)} passed")
    print(f"Rust ({RUST_BINARY.name}): {rust_passed}/{len(rust_tests)} passed")
    print(f"\nTotal: {python_passed + rust_passed}/{len(tests)} passed")

    # Detailed results
    print("\n" + "-" * 80)
    print("DETAILED RESULTS")
    print("-" * 80)

    for test in tests:
        status = "PASS" if test.passed else "FAIL"
        print(f"\n[{status}] {test.name}")
        print(f"  Args: {' '.join(test.args)}")
        if test.expected_exit_code is not None:
            print(f"  Expected exit: {test.expected_exit_code}, Got: {test.actual_exit_code}")
        if test.error:
            print(f"  Error: {test.error}")
        if verbose:
            if test.actual_output:
                print(f"  Output: {test.actual_output[:500]}")
            if test.actual_error:
                print(f"  Stderr: {test.actual_error[:500]}")

    # Failures summary
    failures = [t for t in tests if not t.passed]
    if failures:
        print("\n" + "-" * 80)
        print("FAILURES SUMMARY")
        print("-" * 80)
        for test in failures:
            print(f"\n{test.name}:")
            print(f"  {test.error}")

    print("\n" + "=" * 80)


def run_test_suite(verbose=False, filter_impl=None, filter_name=None):
    """Run the complete test suite"""
    print("Creating test cases...")
    tests = create_test_cases()

    # Apply filters
    if filter_impl:
        tests = [t for t in tests if filter_impl.lower() in t.name.lower()]
    if filter_name:
        tests = [t for t in tests if filter_name.lower() in t.name.lower()]

    print(f"Running {len(tests)} tests...\n")

    # Check if implementations exist
    impls_to_check = {
        "Python": PYTHON_PLUGIN,
        "Rust": RUST_BINARY
    }

    for impl_name, impl_path in impls_to_check.items():
        if not impl_path.exists():
            print(f"WARNING: {impl_name} implementation not found at {impl_path}")
            print(f"  Skipping all {impl_name} tests")

    # Run all tests
    for test in tests:
        # Check if implementation exists before running
        if test.implementation == PYTHON_PLUGIN and not PYTHON_PLUGIN.exists():
            test.passed = False
            test.error = "Python implementation not found"
            continue
        if test.implementation == RUST_BINARY and not RUST_BINARY.exists():
            test.passed = False
            test.error = "Rust implementation not found"
            continue

        test.run()

    # Print results
    print_results(tests, verbose)

    # Return exit code
    failures = sum(1 for t in tests if not t.passed)
    return failures


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Test suite for nagios-plugin-mongodb")
    parser.add_argument("-v", "--verbose", action="store_true", help="Show verbose output")
    parser.add_argument("--python", action="store_true", help="Run only Python tests")
    parser.add_argument("--rust", action="store_true", help="Run only Rust tests")
    parser.add_argument("--filter", type=str, help="Filter test names")

    args = parser.parse_args()

    filter_impl = None
    if args.python:
        filter_impl = "Python"
    elif args.rust:
        filter_impl = "Rust"

    failures = run_test_suite(
        verbose=args.verbose,
        filter_impl=filter_impl,
        filter_name=args.filter
    )

    sys.exit(1 if failures > 0 else 0)
