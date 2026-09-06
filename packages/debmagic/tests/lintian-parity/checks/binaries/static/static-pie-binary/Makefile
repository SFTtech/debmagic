all: hello

ARCH := $(shell dpkg-architecture -qDEB_HOST_ARCH)

ifneq (,$(filter $(ARCH), amd64 arm64 s390x riscv64 loong64))
hello: hello.c
	gcc -static-pie hello.c -o hello
else
hello:
	touch hello
endif

clean:
	rm -f hello

distclean: clean
