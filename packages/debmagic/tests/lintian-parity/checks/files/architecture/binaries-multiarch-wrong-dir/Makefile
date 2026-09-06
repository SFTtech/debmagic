
# Since dpkg in stable (Squeeze) does not support MULTIARCH we cannot use
#  dpkg-architecture -qDEB_HOST_MULTIARCH
# Therefore this test is hardcoded to x86 and amd64

ARCH=$(shell dpkg-architecture -qDEB_HOST_ARCH)

ifeq (i386,$(ARCH))
    WRONG_TRIPLET = x86_64-linux-gnu
else
    WRONG_TRIPLET = i386-linux-gnu
endif


all:
	gcc $(CFLAGS) $(CPPFLAGS) $(LDFLAGS) -fPIC -shared -Wl,-z,defs -Wl,-soname,libbasic.so.2 -o libbasic.so.2 basic.c

install:
	# Install it once under the wrong dir
	install -d $(DESTDIR)/usr/lib/$(WRONG_TRIPLET)
	install -m 644 -c -s libbasic.so.2 $(DESTDIR)/usr/lib/$(WRONG_TRIPLET)/libbasic.so.2

clean distclean:
	rm -f libbasic.so.2

check test:
