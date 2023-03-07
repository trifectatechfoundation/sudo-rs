# Compliance tests

This directory contains compliance tests where we check that the `sudo-rs` command line tool behaves as the original `sudo` for the use cases that we support.

## Dependencies

To run these tests you need to have docker and docker-buildx installed and the docker daemon must be running.
On Arch Linux you can install the relevant packages with the following command:

``` console
$ sudo pacman -S docker docker-buildx
```

## Running the tests

To run the compliance tests against the original sudo execute the following command from this directory:

``` console
$ cargo test -p sudo-compliance-tests
```

To run the tests against sudo-rs set the `SUDO_UNDER_TEST` variable to `ours` before invoking Cargo:

``` console
$ SUDO_UNDER_TEST=ours cargo test -p sudo-compliance-tests
```
