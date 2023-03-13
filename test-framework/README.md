# Compliance tests

This directory contains compliance tests where we check that the `sudo-rs` command line tool behaves as the original `sudo` for the use cases that we support.

## Dependencies

To run these tests you need to have docker and docker-buildx installed and the docker daemon must be running.
On Arch Linux you can install the relevant packages with the following command:

``` console
$ sudo pacman -S docker docker-buildx
```

## Running the tests

To run all the compliance tests against the original sudo execute the following command from this directory:

``` console
$ cargo test -p sudo-compliance-tests -- --include-ignored
```

To run the "gated" compliance tests against sudo-rs set the `SUDO_UNDER_TEST` variable to `ours` before invoking Cargo:

``` console
$ SUDO_UNDER_TEST=ours cargo test -p sudo-compliance-tests
```

## Gating CI on selected tests

Tests (`#[test]` functions) that exercise behavior not yet implemented in sudo-rs MUST be marked as `#[ignored]`.
When said behavior is implemented in sudo-rs, the `#[ignored]` attribute MUST be removed from the test.
CI will run `#[ignored]` tests against sudo-rs and fail the build if any of them passes -- as that indicates that an `#[ignored]` attribute was not removed.
