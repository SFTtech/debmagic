# This test works on amd64 when the cross-compiler for armhf is installed.
#
# The build prerequisite was not added to Lintian, however, since it was
# not clear how the architecture would be enabled in the Gitlab CI   
# runner.
#
# On amd64 or i386, please follow these steps to run the test:
#
#    dpkg --add-architecture armhf
#    apt update
#    apt install gcc-arm-linux-gnueabihf
#
# On all other architectures this may work, but was not tested:
#
#    dpkg --add-architecture amd64
#    apt update
#    apt install gcc-x86-64-linux-gnu
#
# (Taken from: https://wiki.debian.org/CrossToolchains)   

ARCH := $(shell dpkg-architecture -qDEB_HOST_ARCH)

ifeq ($(ARCH),amd64)
CC := arm-linux-gnueabihf-gcc
else
CC := x86_64-linux-gnu-gcc
endif

foreign-binary: hello.c
	$(CC) $^ -o $@

.PHONY: clean
clean:
	rm -f foreign-binary

.PHONY: clean
distclean: clean
