DATAFILE=data-$(shell dpkg-architecture -qDEB_HOST_ARCH)

all:
	echo $(DATAFILE) > $(DATAFILE)

install:
	install -d $(DESTDIR)/usr/lib/pkg-same/ \
		   $(DESTDIR)/usr/lib/debug/pkg-same/
	install -m 0644 $(DATAFILE) $(DESTDIR)/usr/lib/pkg-same/$(DATAFILE)
	install -m 0644 $(DATAFILE) $(DESTDIR)/usr/lib/debug/pkg-same/$(DATAFILE)
