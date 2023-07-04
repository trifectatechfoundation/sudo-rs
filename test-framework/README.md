# Compliance tests

This directory contains compliance tests where we check that the `sudo-rs` command line tool behaves as the original `sudo` for the use cases that we support.
It also contains end-to-end (E2E) tests; these tests are _not_ run against the original `sudo` but use the same test framework and helpers as the compliance tests.

## Dependencies

To run these tests you need to have docker and docker-buildx installed and the docker daemon must be running.
On Arch Linux you can install the relevant packages with the following command:

```console
$ sudo pacman -S docker docker-buildx
```

## Running the tests

To run all the compliance tests against the original sudo execute the following command from this directory:

```console
$ cargo test -p sudo-compliance-tests -- --include-ignored
```

To run the "gated" compliance tests against sudo-rs set the `SUDO_UNDER_TEST` variable to `ours` before invoking Cargo:

```console
$ SUDO_UNDER_TEST=ours cargo test -p sudo-compliance-tests
```

To run the E2E tests, you must set the `SUDO_UNDER_TEST` variable to `ours`:

```console
$ SUDO_UNDER_TEST=ours cargo test -p e2e-tests
```

## Verbose docker build

The first unit test that runs will build a docker image that the rest of unit tests will use.
To print the output of the `docker build` command set the `SUDO_TEST_VERBOSE_DOCKER_BUILD` variable.

```console
$ SUDO_TEST_VERBOSE_DOCKER_BUILD=1 cargo test -p sudo-compliance-tests -- --include-ignored
```

## Gating CI on selected tests

Tests (`#[test]` functions) that exercise behavior not yet implemented in sudo-rs MUST be marked as `#[ignored]`.
When said behavior is implemented in sudo-rs, the `#[ignored]` attribute MUST be removed from the test.
CI will run `#[ignored]` tests against sudo-rs and fail the build if any of them passes -- as that indicates that an `#[ignored]` attribute was not removed.

## Using docker containers as a sudo playground

### Original sudo

_After_ you have run `cargo t -p sudo-compliance-tests` you'll be able to spin up a container based on the docker image used to run the tests against the original sudo with the following command:

```console
$ docker run --rm -it sudo-test-og

root@5b3a062d6dcc:/#
```

After you have an active container you can spawn shells as regular users using the following command

```console
$ docker exec -u 1000:100 -it 5b3a062d6dcc bash

I have no name!@5b3a062d6dcc:/$
```

`1000` is the user ID you'll assume and `100` is the group ID. `5b3a062d6dcc` is the ID of the docker container. You can get the ID from either the shell prompt you get after running `docker run` or from `docker ps`.

Note that terminating the `docker run` shell will terminate all the `docker exec` shells. The docker container will be removed after the `docker run` shell is terminated.

### sudo-rs

_After_ you have run `SUDO_UNDER_TEST=ours cargo t -p sudo-compliance-tests` a Docker image that has a source build of `sudo-rs` installed as the system sudo (`/usr/src/bin/sudo`) will become available. The docker image is named `sudo-test-rs`.

All the instructions in the previous subsection can be used to test `sudo-rs` within a docker container: simply use `sudo-test-rs` instead of `sudo-test-og` as the docker image name.

```console
$ docker run --rm -it sudo-test-rs

root@b5ee5351b9c6:/usr/src/sudo#
```

```console
$ docker exec -u 1000:100 -it b5ee5351b9c6 bash

I have no name!@b5ee5351b9c6:/usr/src/sudo$
```
