ARCH=$(shell dpkg-architecture -qDEB_HOST_MULTIARCH)
ifeq ($(ARCH), x86_64-linux-gnu)
 ARCH_CROSS=i386-linux-gnu
else
 ARCH_CROSS=x86_64-linux-gnu
endif

all:
	find generated -name '*.pc.in' -exec sh -c 'echo "build" {} && cat {} | sed s/\$$\(ARCH\)/$(ARCH)/g | sed s/\$$\(ARCH_CROSS\)/$(ARCH_CROSS)/g > `echo {} | sed s/\.pc\.in$$/\.pc/g`' \;

install:
	install -d $(DESTDIR)/usr/lib/pkgconfig/
	install -d $(DESTDIR)/usr/lib/$(ARCH)/pkgconfig/
	install -m 644 indep-*.pc $(DESTDIR)/usr/lib/pkgconfig/
	install -m 644 generated/arch-*.pc $(DESTDIR)/usr/lib/$(ARCH)/pkgconfig/
	install -m 644 generated/indep-*.pc $(DESTDIR)/usr/lib/pkgconfig/

clean distclean:
	rm -f generated/*.pc

check test:
